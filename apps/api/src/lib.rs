mod blueprint_resolver;
pub mod http;
pub mod model;
mod projection;
pub mod repository;

use sqlx::migrate::Migrator;

pub static MIGRATOR: Migrator = sqlx::migrate!("./migrations");
