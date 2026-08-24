use axum::{
    extract::{Request, State},
    http::Method,
    middleware::Next,
    response::Response,
};
use serde_json::{Map, Value, json};
use uuid::Uuid;

use super::{AppState, auth::AuthenticatedPrincipal};

const REQUEST_ID_HEADER: &str = "x-request-id";
const CORRELATION_ID_HEADER: &str = "x-correlation-id";

/// Records every catalog write attempt after its authorization middleware has
/// produced a response. Metadata is constructed only from server-controlled
/// route information; `redact_metadata` is retained as a guard for future
/// caller-supplied metadata.
pub(super) async fn record(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    if !is_mutation(request.method()) {
        return next.run(request).await;
    }

    let request_id = parse_or_generate_id(
        request
            .headers()
            .get(REQUEST_ID_HEADER)
            .and_then(|value| value.to_str().ok()),
    );
    let correlation_id = parse_or_generate_id(
        request
            .headers()
            .get(CORRELATION_ID_HEADER)
            .and_then(|value| value.to_str().ok()),
    );
    let actor_user_id = request
        .extensions()
        .get::<AuthenticatedPrincipal>()
        .map(|principal| principal.0);
    let workspace_id = request
        .headers()
        .get("x-catalog-workspace-id")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse().ok())
        .filter(|workspace_id| *workspace_id == state.workspace_id);
    let route = request
        .extensions()
        .get::<axum::extract::MatchedPath>()
        .map(|path| path.as_str().to_owned())
        .unwrap_or_else(|| "unmatched".to_owned());
    let method = request.method().clone();
    let (authorization_scope, target) = audit_context(&method, &route, request.uri().path());
    let action = action(&method, &route);
    let response = next.run(request).await;
    let outcome = match response.status().as_u16() {
        200..=399 => "success",
        401 | 403 => "denied",
        _ => "failure",
    };

    // A request without a trusted workspace is not allowed to create an event:
    // RLS would reject it anyway and retaining an untrusted tenant identifier
    // would be worse than losing a malformed request's telemetry.
    if let Some(workspace_id) = workspace_id {
        if let Err(error) = state
            .repository
            .record_audit_event(
                workspace_id,
                actor_user_id,
                request_id,
                correlation_id,
                &action,
                authorization_scope,
                target,
                outcome,
                redact_metadata(json!({ "method": method.as_str(), "route": route })),
            )
            .await
        {
            tracing::error!(%error, %request_id, "could not persist audit event");
        }
    }
    response
}

fn is_mutation(method: &Method) -> bool {
    matches!(
        *method,
        Method::POST | Method::PUT | Method::PATCH | Method::DELETE
    )
}

fn parse_or_generate_id(value: Option<&str>) -> Uuid {
    value
        .and_then(|value| value.parse().ok())
        .unwrap_or_else(Uuid::new_v4)
}

fn action(method: &Method, route: &str) -> String {
    let resource = route
        .trim_start_matches('/')
        .replace("{", "")
        .replace("}", "")
        .replace('/', ".")
        .replace('-', "_");
    let verb = match *method {
        Method::POST => "create_or_apply",
        Method::PUT | Method::PATCH => "update",
        Method::DELETE => "delete",
        _ => "mutate",
    };
    format!("catalog.{resource}.{verb}")
}

fn audit_context(method: &Method, route: &str, path: &str) -> (Value, Value) {
    let permission = if route.starts_with("/blueprints") {
        if route.ends_with("/publish") {
            "blueprints.publish"
        } else {
            "blueprints.write"
        }
    } else if route.starts_with("/contexts") {
        "contexts.write"
    } else if method == Method::DELETE {
        "entities.delete"
    } else {
        "entities.write"
    };
    let segments: Vec<_> = path
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect();
    let identifier = segments
        .iter()
        .find_map(|segment| segment.parse::<Uuid>().ok());
    (
        json!({ "permission": permission }),
        match identifier {
            Some(id) => json!({ "id": id }),
            None => json!({ "route": route }),
        },
    )
}

pub(crate) fn redact_metadata(value: Value) -> Value {
    match value {
        Value::Object(object) => Value::Object(
            object
                .into_iter()
                .map(|(key, value)| {
                    let redacted = is_secret_key(&key);
                    (
                        key,
                        if redacted {
                            Value::String("[REDACTED]".to_owned())
                        } else {
                            redact_metadata(value)
                        },
                    )
                })
                .collect::<Map<_, _>>(),
        ),
        Value::Array(values) => Value::Array(values.into_iter().map(redact_metadata).collect()),
        value => value,
    }
}

fn is_secret_key(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    [
        "password",
        "secret",
        "token",
        "authorization",
        "credential",
        "api_key",
        "apikey",
    ]
    .iter()
    .any(|needle| key.contains(needle))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_secrets_recursively_without_losing_safe_metadata() {
        assert_eq!(
            redact_metadata(json!({
                "password": "not retained",
                "nested": { "accessToken": "also not retained", "target": "entity" },
                "items": [{ "api_key": "nope" }],
            })),
            json!({
                "password": "[REDACTED]",
                "nested": { "accessToken": "[REDACTED]", "target": "entity" },
                "items": [{ "api_key": "[REDACTED]" }],
            })
        );
    }
}
