mod blueprint_resolver;
pub mod http;
pub mod model;
pub mod repository;
pub mod telemetry;

use sqlx::migrate::Migrator;

pub static MIGRATOR: Migrator = sqlx::migrate!("./migrations");
