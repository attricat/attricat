use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::{migrate::Migrator, postgres::PgPoolOptions};
use uuid::Uuid;

static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

// Entity is the "instance" of an item in the system
struct Entity {
    id: Uuid,
    code: String,
    blueprint_id: Uuid,
    blueprint_version: i64,
    // entity as JSONB, for fast retrieval and quick search - depending on context. eg. "search", "preview" etc.
    projections: Value,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    deleted_at: Option<DateTime<Utc>>,
}

// Attribute describes the field (pure metadata)
struct Attribute {
    id: Uuid,
    blueprint_id: Uuid,
    blueprint_version: i64,
    code: String,
    value_type: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    deleted_at: Option<DateTime<Utc>>,
}

// Value is an entity attribute value.
struct AttributeValue {
    id: Uuid,
    entity_id: Uuid,
    attribute_id: Uuid,
    value: Value,
    // Set for relationship attribute types; the value row retains the relationship context/history.
    relationship_target_entity_id: Option<Uuid>,
    // Can describe things such as "lang", currency, channel... etc - so we can have Dimensions
    context_id: Option<Uuid>,

    created_at: DateTime<Utc>,
    // we support value versioning
}

// Attribute value context, assigned to every value (optionally)
struct AttributeContext {
    id: Uuid,
    code: String,
    // JSONB describing context data - {lang, currency, channel... etc.} This will be very
    // permissive.
    data: Value,
}

// Blueprint describes how an Entity is structured (what fields does it have)
// The definition field will be TOML, describing what fields we have, validation rules on them, the
// order etc.
// It will be used to generate Attributes.
struct Blueprint {
    id: Uuid,
    name: String,
    version: i64,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    deleted_at: Option<DateTime<Utc>>,
    // TOML value describing definition. This will hold attribute translations also (like labels).
    definition: String,
    // Change detection
    definition_hash: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    let database_url =
        std::env::var("DATABASE_URL").map_err(|_| "DATABASE_URL must be set to start the API")?;
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await?;

    MIGRATOR.run(&pool).await?;
    println!("database migrations are up to date");

    Ok(())
}
