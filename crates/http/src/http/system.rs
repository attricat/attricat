use super::AppState;
use axum::{Json, extract::State};
use serde::Serialize;

/// Identifies the build serving requests. The binary embeds these values at
/// compile time, so they describe the running process rather than the
/// current state of any checkout.
#[derive(Clone, Copy, Debug, Serialize)]
pub struct BuildInfo {
    pub version: &'static str,
    pub branch: &'static str,
    pub commit: &'static str,
}

#[derive(Serialize)]
pub(super) struct SystemHealth {
    build: BuildInfo,
}

pub(super) async fn health(State(state): State<AppState>) -> Json<SystemHealth> {
    Json(SystemHealth {
        build: state.build_info,
    })
}
