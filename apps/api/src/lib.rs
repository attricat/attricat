pub mod account;
mod blueprint_resolver;
pub mod constants;
pub mod file_access;
pub mod file_worker;
pub mod http;
pub mod mail;
pub mod model;
pub mod repository;
pub mod storage;
pub mod telemetry;

use sqlx::migrate::Migrator;

pub static MIGRATOR: Migrator = sqlx::migrate!("./migrations");
