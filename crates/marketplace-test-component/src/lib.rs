//! A side-loadable test exporter for Shopify (Admin GraphQL), Amazon
//! (Selling Partner API listings and JSON feeds) and Allegro (product
//! offers), built only on the mediated `attricat:host@1.1.0` APIs. It runs as
//! a connector job: connector page v2 rows with expanded variants go to the
//! destination of the job's connection, and the withdrawn feed closes items
//! out. It never sees a credential: connections inject tokens and secrets.
//!
//! It is a host-platform test fixture, not a production integration; it
//! targets the local marketplace mock in `examples/marketplace-export`.

wit_bindgen::generate!({
    path: "../extension-runtime/wit-host",
    world: "outcome-extension",
});

use attricat::host::{api, artifacts, attricat as catalog, clock, outcomes, transfer};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use exports::attricat::host::{
    command_handler::{self, CommandRequest, CommandResponse},
    deferrable_operations::{self, BatchOutcome, OperationRequest},
    event_handler::{self, Delivery},
};
use serde_json::{Map, Value, json};

struct Component;

type Failure = outcomes::Failure;

/// Rows one batch reads; each may become several destination requests.
const PAGE_LIMIT: u32 = 8;
/// How long a push holds a record's lease.
const LEASE_MS: u64 = 10 * 60 * 1000;
/// Public image links must outlive asynchronous marketplace fetches.
const IMAGE_LINK_SECONDS: u64 = 6 * 60 * 60;
const FEED_OUTPUT: &str = "listings-feed.json";
const FEED_CONTENT_TYPE: &str = "application/json; charset=UTF-8";
const ALLEGRO_MEDIA_TYPE: &str = "application/vnd.allegro.public.v1+json";

fn parse(value: &str) -> Result<Value, String> {
    serde_json::from_str(value).map_err(|error| error.to_string())
}

fn call(operation: &str, request: Value) -> Result<Value, String> {
    parse(&api::call(operation, &request.to_string())?)
}

fn fault(message: impl ToString) -> Failure {
    Failure::Fault(message.to_string())
}

fn retry(message: impl Into<String>, after_ms: Option<u64>) -> Failure {
    Failure::Retry(outcomes::Retry {
        message: message.into(),
        after_ms,
    })
}

fn log(message: &str) {
    let _ = api::log("info", message);
}

/// A tiny stable digest of what a listing sends.
fn content_hash(value: &Value) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in value.to_string().bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{hash:016x}")
}

// ---- brokered HTTP ---------------------------------------------------------

/// What a brokered request ended with.
enum Http {
    Response {
        status: u16,
        headers: Value,
        body: Value,
    },
    /// The host's request budget is spent; wait this long.
    RateLimited(u64),
    /// The request may or may not have reached the destination.
    Uncertain,
    /// The connection's token could not be refreshed right now.
    Unavailable(String),
}

fn http(
    rule: &str,
    method: &str,
    url: &str,
    connection: Option<&str>,
    headers: Value,
    body: Option<String>,
) -> Result<Http, String> {
    let mut request = json!({
        "host_permission_id": rule,
        "method": method,
        "url": url,
        "headers": headers,
    });
    if let Some(connection) = connection {
        request["connection"] = json!(connection);
    }
    if let Some(body) = body {
        request["body"] = json!(body);
    }
    let response = call("network.request.v2", request)?;
    Ok(match response["outcome"].as_str() {
        Some("response") => {
            let bytes = BASE64
                .decode(response["body_base64"].as_str().unwrap_or_default())
                .unwrap_or_default();
            let text = String::from_utf8_lossy(&bytes).into_owned();
            Http::Response {
                status: response["status"].as_u64().unwrap_or(0) as u16,
                headers: response["headers"].clone(),
                body: serde_json::from_str(&text).unwrap_or(Value::String(text)),
            }
        }
        Some("rate_limited") => {
            Http::RateLimited(response["retry_after_ms"].as_u64().unwrap_or(1000))
        }
        Some("connection_unavailable") => {
            Http::Unavailable(response["message"].as_str().unwrap_or_default().into())
        }
        _ => Http::Uncertain,
    })
}

/// Milliseconds a `Retry-After` header asks for, or a default.
fn retry_after(headers: &Value, default: u64) -> u64 {
    headers["retry-after"]
        .as_str()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .map_or(default, |seconds| seconds * 1000)
}

// ---- destinations ------------------------------------------------------------

#[derive(Clone, Copy, PartialEq)]
enum Marketplace {
    Shopify,
    Amazon,
    Allegro,
}

/// The connection a run or command pushes to.
struct Target {
    marketplace: Marketplace,
    connection: String,
    parameters: Value,
}

impl Target {
    fn load(connection: &str) -> Result<Self, String> {
        let described = call("connections.get.v1", json!({"connection": connection}))?;
        let marketplace = match described["kind"].as_str() {
            Some("shopify") => Marketplace::Shopify,
            Some("amazon") => Marketplace::Amazon,
            Some("allegro") => Marketplace::Allegro,
            other => return Err(format!("unknown connection kind {other:?}")),
        };
        Ok(Self {
            marketplace,
            connection: connection.into(),
            parameters: described["parameters"].clone(),
        })
    }

    fn parameter(&self, name: &str, default: &str) -> String {
        self.parameters[name].as_str().unwrap_or(default).to_owned()
    }

    fn base(&self) -> String {
        self.parameter(
            "api_base",
            match self.marketplace {
                Marketplace::Shopify => "https://shop.mock.test",
                Marketplace::Amazon => "https://sellingpartnerapi.mock.test",
                Marketplace::Allegro => "https://allegro.mock.test",
            },
        )
    }

    fn connection(&self) -> Option<&str> {
        Some(&self.connection)
    }
}

/// A file a record references, as connector page v2 returns it.
#[derive(Clone)]
struct FileRef {
    file_id: String,
    filename: String,
    mime_type: String,
}

#[derive(Clone)]
struct Variant {
    sku: String,
    size: String,
    price: Option<f64>,
}

/// What one record exports.
#[derive(Clone)]
struct Listing {
    record_id: String,
    revision: i64,
    sku: String,
    title: String,
    description: String,
    price: Option<f64>,
    images: Vec<FileRef>,
    variants: Vec<Variant>,
}

impl Listing {
    fn from_values(
        record_id: &str,
        revision: i64,
        values: &Value,
        files: &Value,
        related: &Value,
    ) -> Self {
        let text = |name: &str| values[name].as_str().unwrap_or_default().to_owned();
        let images = files["image"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|file| FileRef {
                file_id: file["file_id"].as_str().unwrap_or_default().into(),
                filename: file["filename"].as_str().unwrap_or("image").into(),
                mime_type: file["mime_type"]
                    .as_str()
                    .unwrap_or("application/octet-stream")
                    .into(),
            })
            .collect();
        let variants = related["variants"]
            .as_array()
            .into_iter()
            .flatten()
            // A variant unpublished in the channel is not listed.
            .filter(|variant| variant["published"] != json!(false))
            .map(|variant| Variant {
                sku: variant["values"]["sku"].as_str().unwrap_or_default().into(),
                size: variant["values"]["size"]
                    .as_str()
                    .unwrap_or_default()
                    .into(),
                price: variant["values"]["price"].as_f64(),
            })
            .filter(|variant| !variant.sku.is_empty())
            .collect();
        Self {
            record_id: record_id.into(),
            revision,
            sku: text("sku"),
            title: text("title"),
            description: text("description"),
            price: values["price"].as_f64(),
            images,
            variants,
        }
    }

    fn from_row(row: &Value) -> Self {
        Self::from_values(
            row["record_id"].as_str().unwrap_or_default(),
            row["content_revision"].as_i64().unwrap_or(0),
            &row["values"],
            &row["files"],
            &row["related"],
        )
    }

    fn summary(&self) -> Value {
        json!({
            "sku": self.sku,
            "title": self.title,
            "description": self.description,
            "price": self.price,
            "images": self.images.iter().map(|image| &image.file_id).collect::<Vec<_>>(),
            "variants": self.variants.iter().map(|variant| json!([variant.sku, variant.size, variant.price])).collect::<Vec<_>>(),
        })
    }

    /// The destination's checks, so a preview shows what a push would reject.
    fn issues(&self, marketplace: Marketplace) -> Vec<String> {
        let mut issues = Vec::new();
        if self.sku.is_empty() {
            issues.push("sku is required".to_owned());
        }
        if self.title.trim().is_empty() {
            issues.push("title is required".to_owned());
        }
        if marketplace != Marketplace::Shopify && self.price.is_none() && self.variants.is_empty() {
            issues.push("price is required".to_owned());
        }
        issues
    }
}

/// The result of pushing one listing.
enum Pushed {
    Synced {
        external_id: String,
        external_ids: Value,
    },
    Rejected {
        code: String,
        message: String,
    },
    /// An asynchronous command was accepted; poll `operation`.
    Pending {
        operation: Value,
    },
    Throttled(u64),
    Uncertain,
}

fn price_text(price: Option<f64>) -> String {
    format!("{:.2}", price.unwrap_or_default())
}

/// A public, expiring link to a record file for a destination that fetches
/// media itself; `None` when the deployment does not offer links.
fn image_link(listing: &Listing) -> Option<String> {
    let image = listing.images.first()?;
    call(
        "files.link.v1",
        json!({"file_id": image.file_id, "record_id": listing.record_id, "expires_in_seconds": IMAGE_LINK_SECONDS}),
    )
    .ok()
    .and_then(|link| link["url"].as_str().map(str::to_owned))
}

/// Maps a response that is not a success to a push result or batch failure.
fn unsuccessful(status: u16, headers: &Value, body: &Value) -> Result<Pushed, Failure> {
    match status {
        429 => Ok(Pushed::Throttled(retry_after(headers, 2000))),
        500..=599 => Err(retry(
            format!("destination returned HTTP {status}"),
            Some(retry_after(headers, 2000)),
        )),
        401 | 403 => Err(retry("destination rejected the credentials", Some(5000))),
        _ => Ok(Pushed::Rejected {
            code: format!("http_{status}"),
            message: body.to_string().chars().take(512).collect(),
        }),
    }
}

fn handle(result: Result<Http, String>) -> Result<Result<(u16, Value, Value), Pushed>, Failure> {
    match result.map_err(fault)? {
        Http::Response {
            status,
            headers,
            body,
        } => Ok(Ok((status, headers, body))),
        Http::RateLimited(after) => Ok(Err(Pushed::Throttled(after))),
        Http::Uncertain => Ok(Err(Pushed::Uncertain)),
        Http::Unavailable(message) => Err(retry(message, Some(10_000))),
    }
}

// ---- Shopify -----------------------------------------------------------------

fn shopify_graphql(target: &Target, query: &str, variables: Value) -> Result<Http, String> {
    let version = target.parameter("api_version", "2026-10");
    http(
        "shopify-admin",
        "POST",
        &format!("{}/admin/api/{version}/graphql.json", target.base()),
        target.connection(),
        json!({"content-type": "application/json"}),
        Some(json!({"query": query, "variables": variables}).to_string()),
    )
}

/// Uploads a record image through a staged upload: Shopify hands out a
/// signed form target; the host posts the record file to it directly.
fn shopify_stage_image(
    target: &Target,
    listing: &Listing,
    image: &FileRef,
) -> Result<Option<String>, Failure> {
    let staged = match handle(shopify_graphql(
        target,
        "mutation stagedUploadsCreate($input: [StagedUploadInput!]!) { stagedUploadsCreate(input: $input) { stagedTargets { url resourceUrl parameters { name value } } userErrors { field message } } }",
        json!({"input": [{"filename": image.filename, "mimeType": image.mime_type, "resource": "IMAGE", "httpMethod": "POST"}]}),
    ))? {
        Ok((200, _, body)) => body,
        _ => return Ok(None),
    };
    let staged_target = &staged["data"]["stagedUploadsCreate"]["stagedTargets"][0];
    let opened = parse(
        &transfer::open_record_file(
            &json!({"record_id": listing.record_id, "file_id": image.file_id}).to_string(),
        )
        .map_err(fault)?,
    )
    .map_err(fault)?;
    let fields: Vec<Value> = staged_target["parameters"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|parameter| json!({"name": parameter["name"], "value": parameter["value"]}))
        .collect();
    let delivered = parse(
        &transfer::deliver_output_v2(
            &json!({
                "host_permission_id": "shopify-staged",
                "url": staged_target["url"],
                "method": "POST",
                "artifact_id": opened["artifact_id"],
                "delivery_key": format!("image-{}-{}", image.file_id, listing.revision),
                "headers": {"content-type": image.mime_type},
                "multipart": {"fields": fields, "file_field": "file", "filename": image.filename},
            })
            .to_string(),
        )
        .map_err(fault)?,
    )
    .map_err(fault)?;
    Ok((delivered["outcome"] == "succeeded")
        .then(|| staged_target["resourceUrl"].as_str().map(str::to_owned))
        .flatten())
}

fn shopify_push(target: &Target, listing: &Listing) -> Result<Pushed, Failure> {
    let mut files = Vec::new();
    for image in &listing.images {
        if let Some(source) = shopify_stage_image(target, listing, image)? {
            files.push(json!({"originalSource": source, "contentType": "IMAGE"}));
        }
    }
    let (options, variants) = if listing.variants.is_empty() {
        (
            json!([{"name": "Title", "values": [{"name": "Default Title"}]}]),
            json!([{"optionValues": [{"optionName": "Title", "name": "Default Title"}], "price": price_text(listing.price), "sku": listing.sku}]),
        )
    } else {
        (
            json!([{"name": "Size", "values": listing.variants.iter().map(|variant| json!({"name": variant.size})).collect::<Vec<_>>()}]),
            Value::Array(
                listing
                    .variants
                    .iter()
                    .map(|variant| json!({"optionValues": [{"optionName": "Size", "name": variant.size}], "price": price_text(variant.price.or(listing.price)), "sku": variant.sku}))
                    .collect(),
            ),
        )
    };
    let handle_name = listing.sku.to_ascii_lowercase();
    let response = handle(shopify_graphql(
        target,
        "mutation productSet($input: ProductSetInput!, $identifier: ProductSetIdentifiers) { productSet(input: $input, identifier: $identifier, synchronous: true) { product { id variants(first: 100) { nodes { id sku } } } userErrors { field message code } } }",
        // Upserting by handle makes a repeated push idempotent.
        json!({
            "identifier": {"handle": handle_name},
            "input": {
                "title": listing.title,
                "handle": handle_name,
                "descriptionHtml": listing.description,
                "status": "ACTIVE",
                "productOptions": options,
                "variants": variants,
                "files": files,
            },
        }),
    ))?;
    let (status, headers, body) = match response {
        Ok(response) => response,
        Err(pushed) => return Ok(pushed),
    };
    if status != 200 {
        return unsuccessful(status, &headers, &body);
    }
    let result = &body["data"]["productSet"];
    if let Some(errors) = result["userErrors"]
        .as_array()
        .filter(|errors| !errors.is_empty())
    {
        return Ok(Pushed::Rejected {
            code: errors[0]["code"]
                .as_str()
                .unwrap_or("user_error")
                .to_owned(),
            message: errors
                .iter()
                .filter_map(|error| error["message"].as_str())
                .collect::<Vec<_>>()
                .join("; "),
        });
    }
    let id = result["product"]["id"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    let variants: Map<String, Value> = result["product"]["variants"]["nodes"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|node| Some((node["sku"].as_str()?.to_owned(), node["id"].clone())))
        .collect();
    Ok(Pushed::Synced {
        external_ids: json!({"product": id, "variants": variants, "handle": handle_name, "media": files.len()}),
        external_id: id,
    })
}

fn shopify_withdraw(target: &Target, mapping: &Value) -> Result<(), Failure> {
    let Some(id) = mapping["external_id"].as_str() else {
        return Ok(());
    };
    handle(shopify_graphql(
        target,
        "mutation productDelete($input: ProductDeleteInput!) { productDelete(input: $input) { deletedProductId userErrors { message } } }",
        json!({"input": {"id": id}}),
    ))?
    .map(drop)
    .map_err(|_| retry("product deletion did not complete", Some(5000)))
}

// ---- Amazon ------------------------------------------------------------------

fn amazon_attributes(
    target: &Target,
    listing: &Listing,
    variant: Option<&Variant>,
    image: Option<&str>,
) -> Value {
    let marketplace = target.parameter("marketplace_id", "A1PA6795UKMFR9");
    let currency = target.parameter("currency", "EUR");
    let mut attributes = json!({
        "item_name": [{"value": listing.title, "marketplace_id": marketplace}],
        "product_description": [{"value": listing.description, "marketplace_id": marketplace}],
    });
    if listing.title.is_empty() {
        attributes["item_name"] = json!([]);
    }
    let price = variant.and_then(|variant| variant.price).or(listing.price);
    if (variant.is_some() || listing.variants.is_empty())
        && let Some(price) = price
    {
        attributes["purchasable_offer"] = json!([{"currency": currency, "marketplace_id": marketplace, "our_price": [{"schedule": [{"value_with_tax": price}]}]}]);
    }
    if let Some(image) = image {
        attributes["main_product_image_locator"] =
            json!([{"media_location": image, "marketplace_id": marketplace}]);
    }
    match variant {
        Some(variant) => {
            attributes["parentage_level"] =
                json!([{"value": "child", "marketplace_id": marketplace}]);
            attributes["child_parent_sku_relationship"] = json!([{"child_relationship_type": "variation", "parent_sku": listing.sku, "marketplace_id": marketplace}]);
            attributes["size"] = json!([{"value": variant.size, "marketplace_id": marketplace}]);
            attributes["variation_theme"] = json!([{"name": "SIZE_NAME"}]);
        }
        None if !listing.variants.is_empty() => {
            attributes["parentage_level"] =
                json!([{"value": "parent", "marketplace_id": marketplace}]);
            attributes["variation_theme"] = json!([{"name": "SIZE_NAME"}]);
            // A parent is not buyable itself; the mock still wants an offer.
            attributes["purchasable_offer"] = json!([{"currency": currency, "marketplace_id": marketplace, "our_price": [{"schedule": [{"value_with_tax": price.or_else(|| listing.variants.iter().find_map(|variant| variant.price)).unwrap_or_default()}]}]}]);
        }
        None => {}
    }
    attributes
}

/// Every listing item a record becomes: its own SKU, then its variants.
fn amazon_items(target: &Target, listing: &Listing, image: Option<&str>) -> Vec<(String, Value)> {
    let mut items = vec![(
        listing.sku.clone(),
        amazon_attributes(target, listing, None, image),
    )];
    for variant in &listing.variants {
        items.push((
            variant.sku.clone(),
            amazon_attributes(target, listing, Some(variant), image),
        ));
    }
    items
}

fn amazon_listing_url(target: &Target, sku: &str) -> String {
    format!(
        "{}/listings/2021-08-01/items/{}/{sku}?marketplaceIds={}&issueLocale=en_US",
        target.base(),
        target.parameter("seller_id", "A1SELLER"),
        target.parameter("marketplace_id", "A1PA6795UKMFR9"),
    )
}

/// The Amazon product type an operator chose in the `export_mapping` panel,
/// which keeps it in extension storage under `field-map:amazon`.
fn amazon_product_type() -> String {
    call("storage.get.v1", json!({"key": "field-map:amazon"}))
        .ok()
        .and_then(|entry| entry["value"]["product_type"].as_str().map(str::to_owned))
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "PRODUCT".into())
}

fn amazon_push(target: &Target, listing: &Listing) -> Result<Pushed, Failure> {
    let image = image_link(listing);
    let mut skus = Vec::new();
    for (sku, attributes) in amazon_items(target, listing, image.as_deref()) {
        let response = handle(http(
            "amazon-sp",
            "PUT",
            &amazon_listing_url(target, &sku),
            target.connection(),
            json!({"content-type": "application/json"}),
            Some(json!({"productType": amazon_product_type(), "requirements": "LISTING", "attributes": attributes}).to_string()),
        ))?;
        let (status, headers, body) = match response {
            Ok(response) => response,
            Err(pushed) => return Ok(pushed),
        };
        if status != 200 {
            return unsuccessful(status, &headers, &body);
        }
        if body["status"] == "INVALID" {
            let issues = body["issues"].as_array().cloned().unwrap_or_default();
            return Ok(Pushed::Rejected {
                code: issues
                    .first()
                    .and_then(|issue| issue["code"].as_str())
                    .unwrap_or("INVALID")
                    .to_owned(),
                message: issues
                    .iter()
                    .filter_map(|issue| issue["message"].as_str())
                    .map(|message| format!("{sku}: {message}"))
                    .collect::<Vec<_>>()
                    .join("; "),
            });
        }
        skus.push(sku);
    }
    Ok(Pushed::Synced {
        external_id: listing.sku.clone(),
        external_ids: json!({"skus": skus, "image": image.is_some()}),
    })
}

/// After an uncertain push, Amazon has no idempotency key: look the SKU up.
fn amazon_exists(target: &Target, sku: &str) -> Result<bool, Failure> {
    match handle(http(
        "amazon-sp",
        "GET",
        &amazon_listing_url(target, sku),
        target.connection(),
        json!({}),
        None,
    ))? {
        Ok((200, _, _)) => Ok(true),
        Ok((404, _, _)) => Ok(false),
        _ => Err(retry("listing lookup did not complete", Some(5000))),
    }
}

fn amazon_withdraw(target: &Target, mapping: &Value) -> Result<(), Failure> {
    for sku in mapping["external_ids"]["skus"]
        .as_array()
        .into_iter()
        .flatten()
    {
        let sku = sku.as_str().unwrap_or_default();
        match handle(http(
            "amazon-sp",
            "DELETE",
            &amazon_listing_url(target, sku),
            target.connection(),
            json!({}),
            None,
        ))? {
            Ok((200 | 404, _, _)) => {}
            _ => return Err(retry("listing deletion did not complete", Some(5000))),
        }
    }
    Ok(())
}

fn sp_api(
    target: &Target,
    method: &str,
    path: &str,
    body: Option<Value>,
) -> Result<Result<(u16, Value, Value), Pushed>, Failure> {
    handle(http(
        "amazon-sp",
        method,
        &format!("{}{path}", target.base()),
        target.connection(),
        json!({"content-type": "application/json"}),
        body.map(|body| body.to_string()),
    ))
}

// ---- Allegro -----------------------------------------------------------------

fn allegro_headers() -> Value {
    json!({"accept": ALLEGRO_MEDIA_TYPE, "content-type": ALLEGRO_MEDIA_TYPE})
}

fn allegro_offer(target: &Target, listing: &Listing) -> Value {
    let images: Vec<String> = image_link(listing).into_iter().collect();
    let price = listing
        .price
        .or_else(|| listing.variants.iter().find_map(|variant| variant.price));
    json!({
        "name": listing.title,
        "productSet": [{"product": {"name": listing.title, "images": images}}],
        "external": {"id": listing.sku},
        "sellingMode": {"format": "BUY_NOW", "price": price.map(|price| json!({"amount": price_text(Some(price)), "currency": target.parameter("currency", "PLN")}))},
        "stock": {"available": listing.variants.len().max(1) * 10},
        "description": {"sections": [{"items": [{"type": "TEXT", "content": format!("<p>{}</p>", listing.description)}]}]},
        "publication": {"status": "ACTIVE"},
    })
}

/// Creates an offer, or updates the one the mapping already names.
fn allegro_submit(
    target: &Target,
    listing: &Listing,
    existing: Option<&str>,
) -> Result<Pushed, Failure> {
    let (method, url) = match existing {
        Some(id) => (
            "PATCH",
            format!("{}/sale/product-offers/{id}", target.base()),
        ),
        None => ("POST", format!("{}/sale/product-offers", target.base())),
    };
    let response = handle(http(
        "allegro-api",
        method,
        &url,
        target.connection(),
        allegro_headers(),
        Some(allegro_offer(target, listing).to_string()),
    ))?;
    let (status, headers, body) = match response {
        Ok(response) => response,
        Err(pushed) => return Ok(pushed),
    };
    match status {
        200 | 201 => {
            let id = body["id"]
                .as_str()
                .or(existing)
                .unwrap_or_default()
                .to_owned();
            Ok(Pushed::Synced {
                external_ids: json!({"offer": id}),
                external_id: id,
            })
        }
        202 => Ok(Pushed::Pending {
            operation: json!({"offer_id": body["id"], "location": headers["location"], "after_ms": retry_after(&headers, 1000)}),
        }),
        422 | 400 => Ok(Pushed::Rejected {
            code: "allegro_validation".into(),
            message: body["errors"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|error| error["message"].as_str())
                .collect::<Vec<_>>()
                .join("; "),
        }),
        _ => unsuccessful(status, &headers, &body),
    }
}

/// Polls an offer command. `Ok(None)` while it is still running.
fn allegro_poll(target: &Target, operation: &Value) -> Result<Option<Pushed>, Failure> {
    let Some(location) = operation["location"].as_str() else {
        return Ok(Some(Pushed::Uncertain));
    };
    let response = handle(http(
        "allegro-api",
        "GET",
        location,
        target.connection(),
        allegro_headers(),
        None,
    ))?;
    let (status, headers, body) = match response {
        Ok(response) => response,
        Err(Pushed::Throttled(_)) => return Ok(None),
        Err(pushed) => return Ok(Some(pushed)),
    };
    let offer_id = operation["offer_id"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    Ok(match status {
        202 => None,
        // A finished command redirects to the offer; the broker returns the
        // redirect instead of following it.
        303 | 200 | 201 => Some(Pushed::Synced {
            external_ids: json!({"offer": offer_id}),
            external_id: offer_id,
        }),
        _ => Some(unsuccessful(status, &headers, &body)?),
    })
}

/// After an uncertain submission, look the offer up by the listing's SKU.
fn allegro_lookup(target: &Target, sku: &str) -> Result<Option<String>, Failure> {
    match handle(http(
        "allegro-api",
        "GET",
        &format!("{}/sale/offers?external.id={sku}", target.base()),
        target.connection(),
        allegro_headers(),
        None,
    ))? {
        Ok((200, _, body)) => Ok(body["offers"][0]["id"].as_str().map(str::to_owned)),
        _ => Err(retry("offer lookup did not complete", Some(5000))),
    }
}

fn allegro_withdraw(target: &Target, mapping: &Value) -> Result<(), Failure> {
    let Some(id) = mapping["external_id"].as_str() else {
        return Ok(());
    };
    match handle(http(
        "allegro-api",
        "PATCH",
        &format!("{}/sale/product-offers/{id}", target.base()),
        target.connection(),
        allegro_headers(),
        Some(json!({"publication": {"status": "ENDED"}}).to_string()),
    ))? {
        Ok((200 | 404, _, _)) => Ok(()),
        _ => Err(retry("offer could not be ended", Some(5000))),
    }
}

// ---- the mapping store -------------------------------------------------------

fn commit(
    target: &Target,
    record_id: &str,
    lease: Option<&Value>,
    update: Value,
) -> Result<Value, Failure> {
    let mut request = update;
    request["connection"] = json!(target.connection);
    request["record_id"] = json!(record_id);
    if let Some(lease) = lease {
        request["lease_token"] = lease.clone();
    }
    call("mappings.commit.v1", request).map_err(|error| {
        // Another worker holds the record; try again after its lease.
        if error.contains("leased by another worker") {
            retry(error, Some(5000))
        } else {
            fault(error)
        }
    })
}

fn release(target: &Target, record_id: &str, lease: &Value) {
    let _ = call(
        "mappings.release.v1",
        json!({"connection": target.connection, "record_id": record_id, "lease_token": lease}),
    );
}

/// Records a push result. Returns whether the batch should stop and wait.
fn record_push(
    target: &Target,
    listing: &Listing,
    hash: &str,
    lease: &Value,
    pushed: Pushed,
    checkpoint: &mut Value,
) -> Result<Option<u64>, Failure> {
    let count = |checkpoint: &mut Value, name: &str| {
        let value = checkpoint["stats"][name].as_u64().unwrap_or(0) + 1;
        checkpoint["stats"][name] = json!(value);
    };
    match pushed {
        Pushed::Synced {
            external_id,
            external_ids,
        } => {
            commit(
                target,
                &listing.record_id,
                Some(lease),
                json!({
                    "status": "synced",
                    "record_revision": listing.revision,
                    "external_id": external_id,
                    "external_ids": external_ids,
                    "content_hash": hash,
                }),
            )?;
            count(checkpoint, "synced");
        }
        Pushed::Rejected { code, message } => {
            log(&format!("{} rejected: {code}", listing.sku));
            commit(
                target,
                &listing.record_id,
                Some(lease),
                json!({
                    "status": "rejected",
                    "record_revision": listing.revision,
                    "error": {"code": code, "message": message},
                }),
            )?;
            count(checkpoint, "rejected");
        }
        Pushed::Pending { operation } => {
            let mut pending = checkpoint["pending"]
                .as_array()
                .cloned()
                .unwrap_or_default();
            pending.push(json!({
                "record_id": listing.record_id,
                "revision": listing.revision,
                "sku": listing.sku,
                "hash": hash,
                "lease": lease,
                "operation": operation,
            }));
            checkpoint["pending"] = Value::Array(pending);
            count(checkpoint, "submitted");
        }
        Pushed::Throttled(after) => {
            release(target, &listing.record_id, lease);
            count(checkpoint, "throttled");
            return Ok(Some(after));
        }
        Pushed::Uncertain => {
            commit(
                target,
                &listing.record_id,
                Some(lease),
                json!({
                    "status": "uncertain",
                    "record_revision": listing.revision,
                    "error": {"code": "uncertain", "message": "the destination may or may not have the change"},
                }),
            )?;
            count(checkpoint, "uncertain");
        }
    }
    Ok(None)
}

/// Pushes one connector row unless the destination already has it.
fn export_row(
    target: &Target,
    row: &Value,
    feed: bool,
    checkpoint: &mut Value,
) -> Result<Option<u64>, Failure> {
    let listing = Listing::from_row(row);
    let begin = call(
        "mappings.begin.v1",
        json!({"connection": target.connection, "record_id": listing.record_id, "record_revision": listing.revision, "lease_ms": LEASE_MS}),
    )
    .map_err(fault)?;
    let count = |checkpoint: &mut Value, name: &str| {
        let value = checkpoint["stats"][name].as_u64().unwrap_or(0) + 1;
        checkpoint["stats"][name] = json!(value);
    };
    match begin["outcome"].as_str() {
        Some("proceed") => {}
        Some("stale") => {
            count(checkpoint, "stale");
            return Ok(None);
        }
        _ => {
            count(checkpoint, "busy");
            return Ok(None);
        }
    }
    let lease = begin["lease_token"].clone();
    let mapping = &begin["mapping"];
    let hash = content_hash(&listing.summary());
    let issues = listing.issues(target.marketplace);
    if listing.sku.is_empty() {
        return record_push(
            target,
            &listing,
            &hash,
            &lease,
            Pushed::Rejected {
                code: "missing_sku".into(),
                message: issues.join("; "),
            },
            checkpoint,
        );
    }
    // Unchanged content needs no push; only the revision moves on.
    if mapping["content_hash"].as_str() == Some(hash.as_str()) && mapping["status"] != "uncertain" {
        commit(
            target,
            &listing.record_id,
            Some(&lease),
            json!({"status": "synced", "record_revision": listing.revision}),
        )?;
        count(checkpoint, "unchanged");
        return Ok(None);
    }
    // An uncertain earlier push is reconciled before pushing again, since
    // Amazon and Allegro have no idempotency keys.
    if mapping["status"] == "uncertain" {
        let found = match target.marketplace {
            Marketplace::Amazon => {
                amazon_exists(target, &listing.sku)?.then(|| listing.sku.clone())
            }
            Marketplace::Allegro => allegro_lookup(target, &listing.sku)?,
            Marketplace::Shopify => None,
        };
        if let Some(external_id) = found {
            count(checkpoint, "reconciled");
            if target.marketplace == Marketplace::Allegro {
                return record_push(
                    target,
                    &listing,
                    &hash,
                    &lease,
                    Pushed::Synced {
                        external_ids: json!({"offer": external_id}),
                        external_id,
                    },
                    checkpoint,
                );
            }
        }
    }
    if feed && target.marketplace == Marketplace::Amazon {
        return feed_row(target, &listing, &hash, &lease, checkpoint);
    }
    let pushed = match target.marketplace {
        Marketplace::Shopify => shopify_push(target, &listing),
        Marketplace::Amazon => amazon_push(target, &listing),
        Marketplace::Allegro => allegro_submit(target, &listing, mapping["external_id"].as_str()),
    };
    match pushed {
        Ok(pushed) => record_push(target, &listing, &hash, &lease, pushed, checkpoint),
        // A failed batch is retried: let the retry take the record again.
        Err(failure) => {
            release(target, &listing.record_id, &lease);
            Err(failure)
        }
    }
}

// ---- Amazon JSON listings feed -------------------------------------------------

/// Adds a record's items to the run's feed document. The lease is released;
/// the revision guard keeps a late result from replacing a newer push.
fn feed_row(
    target: &Target,
    listing: &Listing,
    hash: &str,
    lease: &Value,
    checkpoint: &mut Value,
) -> Result<Option<u64>, Failure> {
    release(target, &listing.record_id, lease);
    let image = image_link(listing);
    let mut messages = checkpoint["feed_batch"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let mut ids = Vec::new();
    for (sku, attributes) in amazon_items(target, listing, image.as_deref()) {
        let id = checkpoint["feed_messages"].as_u64().unwrap_or(0) + 1;
        checkpoint["feed_messages"] = json!(id);
        ids.push(id);
        messages.push(json!({"messageId": id, "sku": sku, "operationType": "UPDATE", "productType": amazon_product_type(), "requirements": "LISTING", "attributes": attributes}));
    }
    checkpoint["feed_batch"] = Value::Array(messages);
    call(
        "storage.set.v1",
        json!({"key": format!("feed:{}:{}", checkpoint["run"].as_str().unwrap_or_default(), listing.record_id), "value": {
            "record_id": listing.record_id, "revision": listing.revision, "hash": hash, "sku": listing.sku,
            "messages": ids, "skus": amazon_items(target, listing, None).into_iter().map(|(sku, _)| sku).collect::<Vec<_>>(),
        }}),
    )
    .map_err(fault)?;
    let value = checkpoint["stats"]["queued"].as_u64().unwrap_or(0) + 1;
    checkpoint["stats"]["queued"] = json!(value);
    Ok(None)
}

/// Appends this batch's feed messages to the staged feed document.
fn flush_feed(
    target: &Target,
    batch_key: &str,
    checkpoint: &mut Value,
    closing: bool,
) -> Result<(), Failure> {
    let messages = checkpoint["feed_batch"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let started = checkpoint["feed_started"] == json!(true);
    if messages.is_empty() && !(closing && started) {
        return Ok(());
    }
    let mut chunk = String::new();
    if !started {
        chunk.push_str(&format!(
            "{{\"header\":{{\"sellerId\":\"{}\",\"version\":\"2.0\",\"issueLocale\":\"en_US\"}},\"messages\":[",
            target.parameter("seller_id", "A1SELLER")
        ));
        checkpoint["feed_started"] = json!(true);
    }
    for (index, message) in messages.iter().enumerate() {
        if started || index > 0 {
            chunk.push(',');
        }
        chunk.push_str(&message.to_string());
    }
    if closing {
        chunk.push_str("]}");
    }
    artifacts::append_output(FEED_OUTPUT, "application/json", batch_key, chunk.as_bytes())
        .map_err(fault)?;
    checkpoint["feed_batch"] = json!([]);
    Ok(())
}

/// Uploads the finished feed document to its presigned URL and creates the
/// feed. Returns the feed ID.
fn submit_feed(
    target: &Target,
    request: &OperationRequest,
    checkpoint: &mut Value,
) -> Result<Option<String>, Failure> {
    flush_feed(target, &request.batch_key, checkpoint, true)?;
    if checkpoint["feed_started"] != json!(true) {
        return Ok(None);
    }
    let artifact_id = artifacts::finalize_output(FEED_OUTPUT).map_err(fault)?;
    let document = match sp_api(
        target,
        "POST",
        "/feeds/2021-06-30/documents",
        Some(json!({"contentType": FEED_CONTENT_TYPE})),
    )? {
        Ok((201 | 200, _, body)) => body,
        Ok((status, _, _)) => {
            return Err(retry(
                format!("feed document creation returned {status}"),
                Some(5000),
            ));
        }
        Err(_) => return Err(retry("feed document creation did not complete", Some(5000))),
    };
    // The presigned PUT is signed for the exact content type and carries no
    // idempotency key; a PUT to the same URL is idempotent anyway.
    let delivered = parse(
        &transfer::deliver_output_v2(
            &json!({
                "host_permission_id": "amazon-s3",
                "url": document["url"],
                "method": "PUT",
                "artifact_id": artifact_id,
                "delivery_key": format!("feed-{}", document["feedDocumentId"].as_str().unwrap_or_default().chars().rev().take(40).collect::<String>()),
                "headers": {"content-type": FEED_CONTENT_TYPE},
            })
            .to_string(),
        )
        .map_err(fault)?,
    )
    .map_err(fault)?;
    if delivered["outcome"] != "succeeded" {
        return Err(fault(format!("feed upload failed: {delivered}")));
    }
    let created = match sp_api(
        target,
        "POST",
        "/feeds/2021-06-30/feeds",
        Some(json!({
            "feedType": "JSON_LISTINGS_FEED",
            "marketplaceIds": [target.parameter("marketplace_id", "A1PA6795UKMFR9")],
            "inputFeedDocumentId": document["feedDocumentId"],
        })),
    )? {
        Ok((200..=202, _, body)) => body,
        _ => return Err(retry("feed creation did not complete", Some(5000))),
    };
    Ok(created["feedId"].as_str().map(str::to_owned))
}

/// Polls the feed; once it is done, reads the result document and records
/// every queued record's outcome. `Ok(false)` while it is processing.
fn finish_feed(
    target: &Target,
    feed_id: &str,
    run_id: &str,
    checkpoint: &mut Value,
) -> Result<bool, Failure> {
    let feed = match sp_api(
        target,
        "GET",
        &format!("/feeds/2021-06-30/feeds/{feed_id}"),
        None,
    )? {
        Ok((200, _, body)) => body,
        _ => return Ok(false),
    };
    if feed["processingStatus"] != "DONE" {
        return Ok(false);
    }
    let result_id = feed["resultFeedDocumentId"].as_str().unwrap_or_default();
    let document = match sp_api(
        target,
        "GET",
        &format!("/feeds/2021-06-30/documents/{result_id}"),
        None,
    )? {
        Ok((200, _, body)) => body,
        _ => return Ok(false),
    };
    let fetched = parse(
        &transfer::fetch_input_v2(
            &json!({
                "host_permission_id": "amazon-s3",
                "url": document["url"],
                "transfer_key": format!("result-{feed_id}"),
                "offset": 0,
                "max_bytes": 1_048_576,
            })
            .to_string(),
        )
        .map_err(fault)?,
    )
    .map_err(fault)?;
    let handle = artifacts::open_input(fetched["artifact_id"].as_str().unwrap_or_default())
        .map_err(fault)?;
    let mut bytes = Vec::new();
    loop {
        let chunk = artifacts::read(&handle, 65536).map_err(fault)?;
        if chunk.is_empty() {
            break;
        }
        bytes.extend(chunk);
    }
    let result: Value = serde_json::from_slice(&bytes).map_err(fault)?;
    let issues = result["issues"].as_array().cloned().unwrap_or_default();
    let mut cursor: Option<String> = None;
    loop {
        let page = call(
            "storage.list.v1",
            json!({"prefix": format!("feed:{run_id}:"), "cursor": cursor, "limit": 100}),
        )
        .map_err(fault)?;
        for entry in page["entries"].as_array().into_iter().flatten() {
            let queued = &entry["value"];
            let ids = queued["messages"].as_array().cloned().unwrap_or_default();
            let problems: Vec<&Value> = issues
                .iter()
                .filter(|issue| ids.contains(&issue["messageId"]))
                .collect();
            let record_id = queued["record_id"].as_str().unwrap_or_default();
            let update = if problems.is_empty() {
                json!({"status": "synced", "record_revision": queued["revision"], "external_id": queued["sku"],
                       "external_ids": {"skus": queued["skus"], "feed": feed_id}, "content_hash": queued["hash"]})
            } else {
                json!({"status": "rejected", "record_revision": queued["revision"], "error": {
                    "code": problems[0]["code"], "message": problems.iter().filter_map(|issue| issue["message"].as_str()).collect::<Vec<_>>().join("; "),
                    "feed": feed_id}})
            };
            // A newer revision synced meanwhile wins; a conflict is not an error.
            match commit(target, record_id, None, update) {
                Ok(_) => {
                    let name = if problems.is_empty() {
                        "synced"
                    } else {
                        "rejected"
                    };
                    let value = checkpoint["stats"][name].as_u64().unwrap_or(0) + 1;
                    checkpoint["stats"][name] = json!(value);
                }
                Err(_) => {
                    let value = checkpoint["stats"]["superseded"].as_u64().unwrap_or(0) + 1;
                    checkpoint["stats"]["superseded"] = json!(value);
                }
            }
            call("storage.delete.v1", json!({"key": entry["key"]})).map_err(fault)?;
        }
        cursor = page["cursor"].as_str().map(str::to_owned);
        if cursor.is_none() {
            break;
        }
    }
    Ok(true)
}

// ---- withdrawals ---------------------------------------------------------------

/// Closes out records withdrawn from the channel. The feed cursor persists
/// per connection, job and channel (a job runs once per channel), so later
/// runs continue where this one stopped.
fn withdraw_batch(target: &Target, scope: &Value, checkpoint: &mut Value) -> Result<bool, Failure> {
    let key = format!(
        "withdrawn-cursor:{}:{}:{}",
        target.connection,
        scope["job_id"].as_str().unwrap_or_default(),
        scope["channel_id"].as_str().unwrap_or_default()
    );
    if checkpoint["withdrawn_cursor"].is_null() {
        let stored = call("storage.get.v1", json!({"key": key})).map_err(fault)?;
        checkpoint["withdrawn_cursor"] = stored["value"].clone();
    }
    let cursor = checkpoint["withdrawn_cursor"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    let page = parse(&catalog::withdrawn(&cursor, 50).map_err(fault)?).map_err(fault)?;
    for item in page["records"].as_array().into_iter().flatten() {
        let record_id = item["record_id"].as_str().unwrap_or_default();
        let mapping = call(
            "mappings.get.v1",
            json!({"connection": target.connection, "record_id": record_id}),
        )
        .map_err(fault)?["mapping"]
            .clone();
        if mapping.is_null() || mapping["status"] == "withdrawn" {
            continue;
        }
        match target.marketplace {
            Marketplace::Shopify => shopify_withdraw(target, &mapping)?,
            Marketplace::Amazon => amazon_withdraw(target, &mapping)?,
            Marketplace::Allegro => allegro_withdraw(target, &mapping)?,
        }
        commit(
            target,
            record_id,
            None,
            json!({"status": "withdrawn", "record_revision": item["record_revision"]}),
        )?;
        let value = checkpoint["stats"]["withdrawn"].as_u64().unwrap_or(0) + 1;
        checkpoint["stats"]["withdrawn"] = json!(value);
    }
    checkpoint["withdrawn_cursor"] = page["next_cursor"].clone();
    call(
        "storage.set.v1",
        json!({"key": key, "value": page["next_cursor"]}),
    )
    .map_err(fault)?;
    Ok(page["more"] != json!(true))
}

// ---- the export operation ------------------------------------------------------

fn progress(checkpoint: &Value) -> String {
    let stats = &checkpoint["stats"];
    let get = |name: &str| stats[name].as_u64().unwrap_or(0);
    json!({
        "completed": get("synced") + get("rejected") + get("unchanged") + get("stale") + get("withdrawn"),
        "outcome": {
            "succeeded": get("synced") + get("withdrawn"),
            "failed": get("rejected") + get("uncertain"),
            "skipped": get("unchanged") + get("stale") + get("busy"),
        },
        "stats": stats,
        "phase": checkpoint["phase"],
    })
    .to_string()
}

fn outcome(checkpoint: Value, done: bool, resume_after_ms: Option<u64>) -> BatchOutcome {
    BatchOutcome {
        progress: progress(&checkpoint),
        checkpoint: checkpoint.to_string(),
        done,
        resume_after_ms,
    }
}

fn process(request: &OperationRequest) -> Result<BatchOutcome, Failure> {
    let mut checkpoint = parse(&request.checkpoint).map_err(fault)?;
    let input = parse(&request.input).map_err(fault)?;
    let scope = call("connector.scope.v1", json!({})).map_err(fault)?;
    let connection = scope["connection"]
        .as_str()
        .ok_or_else(|| fault("export jobs need a connection"))?;
    let target = Target::load(connection).map_err(fault)?;
    let feed = input["feed"] == json!(true) && target.marketplace == Marketplace::Amazon;
    if checkpoint["phase"].is_null() {
        checkpoint["phase"] = json!("rows");
        checkpoint["run"] = json!(request.run_id);
        checkpoint["mode"] = scope["mode"].clone();
        checkpoint["started_at"] = json!(clock::now().seconds);
        checkpoint["stats"] = json!({});
    }
    match checkpoint["phase"].as_str().unwrap_or("rows") {
        "rows" => {
            let cursor = checkpoint["cursor"].as_str().unwrap_or_default().to_owned();
            let page = parse(
                &catalog::page_v2(
                    &json!({"cursor": cursor, "limit": PAGE_LIMIT, "expand": ["variants"]})
                        .to_string(),
                )
                .map_err(fault)?,
            )
            .map_err(fault)?;
            for row in page["rows"].as_array().into_iter().flatten() {
                if let Some(after) = export_row(&target, row, feed, &mut checkpoint)? {
                    // Throttled: keep the cursor, defer, and redo the rest.
                    if feed {
                        flush_feed(&target, &request.batch_key, &mut checkpoint, false)?;
                    }
                    return Ok(outcome(checkpoint, false, Some(after)));
                }
            }
            if feed {
                flush_feed(&target, &request.batch_key, &mut checkpoint, false)?;
            }
            match page["next_cursor"].as_str() {
                Some(next) => checkpoint["cursor"] = json!(next),
                None => {
                    checkpoint["phase"] = json!(if feed {
                        "feed_submit"
                    } else if checkpoint["pending"]
                        .as_array()
                        .is_some_and(|pending| !pending.is_empty())
                    {
                        "poll"
                    } else {
                        "withdrawn"
                    });
                }
            }
            Ok(outcome(checkpoint, false, None))
        }
        "feed_submit" => {
            match submit_feed(&target, request, &mut checkpoint)? {
                Some(feed_id) => {
                    checkpoint["feed_id"] = json!(feed_id);
                    checkpoint["phase"] = json!("feed_poll");
                    // Amazon processes feeds asynchronously: come back later.
                    Ok(outcome(checkpoint, false, Some(1000)))
                }
                None => {
                    checkpoint["phase"] = json!("withdrawn");
                    Ok(outcome(checkpoint, false, None))
                }
            }
        }
        "feed_poll" => {
            let feed_id = checkpoint["feed_id"]
                .as_str()
                .unwrap_or_default()
                .to_owned();
            let polls = checkpoint["feed_polls"].as_u64().unwrap_or(0) + 1;
            checkpoint["feed_polls"] = json!(polls);
            if finish_feed(&target, &feed_id, &request.run_id, &mut checkpoint)? {
                checkpoint["phase"] = json!("withdrawn");
                Ok(outcome(checkpoint, false, None))
            } else {
                Ok(outcome(checkpoint, false, Some(1000)))
            }
        }
        "poll" => {
            let pending = checkpoint["pending"]
                .as_array()
                .cloned()
                .unwrap_or_default();
            let mut waiting = Vec::new();
            let mut after = 1000;
            for item in pending {
                match allegro_poll(&target, &item["operation"])? {
                    None => {
                        after = after.max(item["operation"]["after_ms"].as_u64().unwrap_or(1000));
                        waiting.push(item);
                    }
                    Some(pushed) => {
                        let listing = Listing {
                            record_id: item["record_id"].as_str().unwrap_or_default().into(),
                            revision: item["revision"].as_i64().unwrap_or(0),
                            sku: item["sku"].as_str().unwrap_or_default().into(),
                            title: String::new(),
                            description: String::new(),
                            price: None,
                            images: Vec::new(),
                            variants: Vec::new(),
                        };
                        record_push(
                            &target,
                            &listing,
                            item["hash"].as_str().unwrap_or_default(),
                            &item["lease"],
                            pushed,
                            &mut checkpoint,
                        )?;
                    }
                }
            }
            let polls = checkpoint["polls"].as_u64().unwrap_or(0) + 1;
            checkpoint["polls"] = json!(polls);
            let done = waiting.is_empty();
            checkpoint["pending"] = Value::Array(waiting);
            if done {
                checkpoint["phase"] = json!("withdrawn");
                Ok(outcome(checkpoint, false, None))
            } else {
                // Offer commands are asynchronous: wait instead of spinning.
                Ok(outcome(checkpoint, false, Some(after)))
            }
        }
        _ => {
            let done = withdraw_batch(&target, &scope, &mut checkpoint)?;
            Ok(outcome(checkpoint, done, None))
        }
    }
}

impl deferrable_operations::Guest for Component {
    fn prepare(_request: OperationRequest) -> Result<String, String> {
        Ok("prepared".into())
    }

    fn start(_request: OperationRequest) -> Result<String, String> {
        Ok("started".into())
    }

    fn process_batch(request: OperationRequest) -> Result<BatchOutcome, Failure> {
        process(&request)
    }

    fn checkpoint(_request: OperationRequest) -> Result<(), String> {
        Ok(())
    }

    fn finish(_request: OperationRequest) -> Result<(), String> {
        Ok(())
    }

    fn cancel(_request: OperationRequest) -> Result<(), String> {
        Ok(())
    }
}

// ---- preview -------------------------------------------------------------------

/// A record's listing from its saved values, for previews outside a run.
fn read_listing(record_id: &str) -> Result<Listing, String> {
    let response = api::read(&api::ReadRequest::Record(api::RecordReference {
        record_id: record_id.into(),
    }))?;
    let record = parse(&response.record)?;
    let blueprint = parse(&response.blueprint)?;
    let values = parse(&response.direct_values)?;
    let mut by_code = Map::new();
    for attribute in blueprint["attributes"].as_array().into_iter().flatten() {
        if let Some(value) = values.as_array().into_iter().flatten().find(|value| {
            value["attribute_id"] == attribute["id"]
                && value["relationship_target_record_id"].is_null()
        }) {
            by_code.insert(
                attribute["code"].as_str().unwrap_or_default().into(),
                value["value"].clone(),
            );
        }
    }
    Ok(Listing::from_values(
        record_id,
        record["content_revision"].as_i64().unwrap_or(0),
        &Value::Object(by_code),
        &json!({}),
        &json!({}),
    ))
}

/// Reads or saves the mapping the `export_mapping` panel edits. The panel
/// sends `{export_id}` to read, or `{export_id, product_type}` to save; an
/// invalid product type is reported in the response, not as an error, so the
/// panel can show it.
fn field_map_command(payload: &Value) -> Result<CommandResponse, String> {
    let export_id = payload["export_id"].as_str().unwrap_or_default();
    if !matches!(export_id, "shopify" | "amazon" | "allegro") {
        return Err("unknown export destination".into());
    }
    let key = format!("field-map:{export_id}");
    let response = match payload.get("product_type").and_then(Value::as_str) {
        Some(product_type)
            if product_type.is_empty()
                || (product_type.len() <= 64
                    && product_type.bytes().all(|byte| {
                        byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_'
                    })) =>
        {
            call(
                "storage.set.v1",
                json!({"key": key, "value": {"version": 1, "product_type": product_type}}),
            )?;
            json!({"product_type": product_type, "saved": true})
        }
        Some(_) => json!({"error": "Use 1 to 64 capital letters, digits or underscores."}),
        None => {
            let stored = call("storage.get.v1", json!({"key": key}))?;
            json!({"product_type": stored["value"]["product_type"].as_str().unwrap_or_default()})
        }
    };
    Ok(CommandResponse {
        payload: response.to_string(),
    })
}

impl command_handler::Guest for Component {
    fn handle_command(request: CommandRequest) -> Result<CommandResponse, String> {
        let payload = parse(&request.payload)?;
        match request.handler.as_str() {
            "preview" => {}
            "field-map" => return field_map_command(&payload),
            _ => return Err("unknown command".into()),
        }
        let listing = read_listing(payload["record_id"].as_str().unwrap_or_default())?;
        let marketplace = match payload["export_id"].as_str() {
            Some("amazon") => Marketplace::Amazon,
            Some("allegro") => Marketplace::Allegro,
            _ => Marketplace::Shopify,
        };
        let target = Target {
            marketplace,
            connection: payload["connection"].as_str().unwrap_or_default().into(),
            parameters: payload["connection"]
                .as_str()
                .and_then(|connection| Target::load(connection).ok())
                .map_or(json!({}), |target| target.parameters),
        };
        let destination_payload = match marketplace {
            Marketplace::Shopify => {
                json!({"title": listing.title, "handle": listing.sku.to_ascii_lowercase(), "descriptionHtml": listing.description,
                                           "variants": [{"sku": listing.sku, "price": price_text(listing.price)}]})
            }
            Marketplace::Amazon => {
                json!({"sku": listing.sku, "productType": amazon_product_type(), "attributes": amazon_attributes(&target, &listing, None, None)})
            }
            Marketplace::Allegro => allegro_offer(&target, &listing),
        };
        Ok(CommandResponse {
            payload: json!({"payload": destination_payload, "issues": listing.issues(marketplace)})
                .to_string(),
        })
    }
}

impl event_handler::Guest for Component {
    fn handle_event(_event: api::Event, _delivery: Delivery) -> Result<(), Failure> {
        // Exports run as connector jobs; operator retries start resync runs.
        Ok(())
    }
}

export!(Component);
