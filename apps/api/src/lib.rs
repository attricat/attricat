pub mod account;
pub mod agent_provider;
pub mod agent_runner;
pub mod agent_service;
pub mod agent_tools;
pub mod agent_worker;
pub mod agents;
mod blueprint_resolver;
pub mod catalog_read_service;
pub mod catalog_service;
pub mod constants;
pub mod domain_events;
pub mod event_dispatcher;
pub mod extension_installer;
pub mod extension_runtime;
pub mod extensions;
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
