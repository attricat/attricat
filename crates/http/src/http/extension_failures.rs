//! An installed extension's recent failures: event deliveries its handlers
//! rejected or gave up on, and operation runs that failed or wait to retry.

use axum::{Json, extract::State};
use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::Value;
use uuid::Uuid;

use attricat_domain::{permissions::Permission, states::RunStatus};

use super::{
    AppState,
    auth::{ActiveWorkspace, AuthenticatedPrincipal, ScopedRepository},
    error::ApiError,
    extractors::{ApiPath, ApiQuery},
    page_cursor::{HandlerPageQuery, Page, PageQuery},
};
use crate::repository::{
    ExtensionRunFailure, HandlerFailure, RUN_DIAGNOSTIC_CODE_FIELD, RUN_DIAGNOSTIC_MESSAGE_FIELD,
};

/// How many failures one page lists.
const FAILURE_PAGE_SIZE: usize = 50;

/// Whether a failed run has stopped or will try again.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum RunFailureState {
    /// Dead-lettered: it runs again only when someone replays it.
    Failed,
    /// It failed and waits to try again.
    Retrying,
}

#[derive(Serialize)]
pub(super) struct RunFailure {
    id: Uuid,
    operation_id: String,
    state: RunFailureState,
    scheduled: bool,
    attempts: i32,
    error_code: Option<String>,
    error_message: Option<String>,
    updated_at: DateTime<Utc>,
}

impl From<ExtensionRunFailure> for RunFailure {
    fn from(run: ExtensionRunFailure) -> Self {
        let (error_code, error_message) =
            diagnostic_parts(run.last_error_code, run.last_error_message);
        Self {
            id: run.id,
            operation_id: run.operation_id,
            state: if run.status == RunStatus::DeadLetter.as_str() {
                RunFailureState::Failed
            } else {
                RunFailureState::Retrying
            },
            scheduled: run.schedule_id.is_some(),
            attempts: run.attempts,
            error_code,
            error_message,
            updated_at: run.updated_at,
        }
    }
}

/// A run's stored error as the code and message people read. The component's
/// own code and text message win over the host's generic code; a diagnostic
/// with other fields is shown whole.
fn diagnostic_parts(
    code: Option<String>,
    diagnostic: Option<String>,
) -> (Option<String>, Option<String>) {
    let Some(diagnostic) = diagnostic else {
        return (code, None);
    };
    let Ok(Value::Object(mut fields)) = serde_json::from_str::<Value>(&diagnostic) else {
        return (code, Some(diagnostic));
    };
    let code = match fields.remove(RUN_DIAGNOSTIC_CODE_FIELD) {
        Some(Value::String(own)) => Some(own),
        _ => code,
    };
    let message = match fields.remove(RUN_DIAGNOSTIC_MESSAGE_FIELD) {
        Some(Value::String(message)) => Some(message),
        _ if fields.is_empty() => None,
        _ => Some(Value::Object(fields).to_string()),
    };
    (code, message)
}

/// The failures name records and carry the extension's error text, so the
/// caller needs a workspace-wide `records.read` grant as well as
/// `extensions.read`, like the export status.
async fn require_records_read(
    state: &AppState,
    principal: &AuthenticatedPrincipal,
    workspace: &ActiveWorkspace,
) -> Result<(), ApiError> {
    principal
        .require_workspace_permission(&state.repository, workspace.0, Permission::RecordsRead)
        .await
}

/// A page of the extension's failed and retrying operation runs.
pub(super) async fn run_failures(
    State(state): State<AppState>,
    ScopedRepository(repository): ScopedRepository,
    principal: AuthenticatedPrincipal,
    workspace: ActiveWorkspace,
    ApiPath(extension_id): ApiPath<String>,
    ApiQuery(query): ApiQuery<PageQuery>,
) -> Result<Json<Page<RunFailure>>, ApiError> {
    require_records_read(&state, &principal, &workspace).await?;
    let rows = repository
        .extension_run_failures(&extension_id, query.before()?, FAILURE_PAGE_SIZE as i64 + 1)
        .await?;
    Ok(Json(
        Page::from_rows(rows, FAILURE_PAGE_SIZE).map(RunFailure::from),
    ))
}

/// A page of the event deliveries the extension's handlers rejected or
/// gave up on.
pub(super) async fn delivery_failures(
    State(state): State<AppState>,
    ScopedRepository(repository): ScopedRepository,
    principal: AuthenticatedPrincipal,
    workspace: ActiveWorkspace,
    ApiPath(extension_id): ApiPath<String>,
    ApiQuery(query): ApiQuery<HandlerPageQuery>,
) -> Result<Json<Page<HandlerFailure>>, ApiError> {
    require_records_read(&state, &principal, &workspace).await?;
    let rows = repository
        .extension_handler_failure_page(
            &extension_id,
            query.before()?,
            FAILURE_PAGE_SIZE as i64 + 1,
        )
        .await?;
    Ok(Json(Page::from_rows(rows, FAILURE_PAGE_SIZE)))
}

#[cfg(test)]
mod tests {
    use super::diagnostic_parts;

    fn parts(code: Option<&str>, diagnostic: Option<&str>) -> (Option<String>, Option<String>) {
        diagnostic_parts(code.map(Into::into), diagnostic.map(Into::into))
    }

    #[test]
    fn a_typed_failure_shows_its_own_code_and_message() {
        assert_eq!(
            parts(
                Some("rejected"),
                Some(r#"{"message":"NIP is invalid","rejection_code":"client_rejected"}"#)
            ),
            (
                Some("client_rejected".into()),
                Some("NIP is invalid".into())
            )
        );
        assert_eq!(
            parts(Some("retry"), Some(r#"{"message":"Fakturownia is down"}"#)),
            (Some("retry".into()), Some("Fakturownia is down".into()))
        );
    }

    #[test]
    fn other_diagnostics_are_shown_whole() {
        assert_eq!(
            parts(Some("operation"), Some(r#"{"status":503}"#)),
            (Some("operation".into()), Some(r#"{"status":503}"#.into()))
        );
        assert_eq!(
            parts(
                Some("operation"),
                Some("operation failure (diagnostics redacted)")
            ),
            (
                Some("operation".into()),
                Some("operation failure (diagnostics redacted)".into())
            )
        );
        assert_eq!(
            parts(Some("rejected"), Some("{}")),
            (Some("rejected".into()), None)
        );
        assert_eq!(parts(None, None), (None, None));
    }
}
