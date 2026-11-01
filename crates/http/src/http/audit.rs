use axum::{extract::Request, http::Method};
use serde_json::{Map, Value, json};
use uuid::Uuid;

use super::auth::AuthenticatedPrincipal;
use crate::repository::AuditContext;
use attricat_domain::permissions::Permission;

const REQUEST_ID_HEADER: &str = "x-request-id";
const CORRELATION_ID_HEADER: &str = "x-correlation-id";

/// Creates server-derived metadata that repository transactions persist before
/// committing a successful mutation. Authorization failures never reach a
/// scoped repository and are intentionally not audited.
pub(super) fn request_context(
    request: &Request,
    principal: AuthenticatedPrincipal,
) -> AuditContext {
    let request_id = request
        .extensions()
        .get::<Uuid>()
        .copied()
        .unwrap_or_else(|| {
            parse_or_generate_id(
                request
                    .headers()
                    .get(REQUEST_ID_HEADER)
                    .and_then(|value| value.to_str().ok()),
            )
        });
    let correlation_id = parse_or_generate_id(
        request
            .headers()
            .get(CORRELATION_ID_HEADER)
            .and_then(|value| value.to_str().ok()),
    );
    let route = request
        .extensions()
        .get::<axum::extract::MatchedPath>()
        .map(|path| super::canonical_route(path.as_str()).to_owned())
        .unwrap_or_else(|| "unmatched".to_owned());
    let method = request.method().clone();
    let (authorization_scope, target) = audit_context(&method, &route, request.uri().path());
    AuditContext {
        actor_user_id: Some(principal.0),
        actor_token_id: principal.1,
        request_id,
        correlation_id,
        action: action(&method, &route),
        authorization_scope,
        target,
        metadata: redact_metadata(json!({ "method": method.as_str(), "route": route })),
        agent: None,
    }
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
    format!("attricat.{resource}.{verb}")
}

fn audit_permission(method: &Method, route: &str) -> Permission {
    if route.starts_with("/v1/records/{record_id}/comments") {
        Permission::RecordsRead
    } else if route == "/extensions/{extension_id}/{contribution_id}/command" {
        Permission::RecordsWrite
    } else if route == "/extensions/{extension_id}/{contribution_id}/operations"
        || route.starts_with("/extension-runs")
    {
        Permission::RecordsRead
    } else if route.starts_with("/solution-packs") || route.starts_with("/presentation-assets") {
        Permission::SolutionPacksManage
    } else if route.contains("connector-jobs")
        || route.starts_with("/extension-registries")
        || route.starts_with("/extensions")
        || route.starts_with("/extension-operation-schedules")
    {
        Permission::ExtensionsManage
    } else if route.starts_with("/workflows") {
        Permission::WorkflowsManage
    } else if route.starts_with("/rules") || route.starts_with("/rule-findings") {
        Permission::RulesManage
    } else if route.starts_with("/lexicon") {
        Permission::BlueprintsWrite
    } else if route.starts_with("/blueprints") {
        if route.ends_with("/publish") || route.ends_with("/sequences/{attribute_code}/advance") {
            Permission::BlueprintsPublish
        } else {
            Permission::BlueprintsWrite
        }
    } else if route.starts_with("/v1/records/{record_id}/publications") {
        Permission::RecordsPublish
    } else if route == "/v1/records/{record_id}/status-unlock" {
        Permission::RecordsUnlock
    } else if route.starts_with("/contexts") || route.starts_with("/publication-channels") {
        Permission::ContextsWrite
    } else if route.starts_with("/personal-access-tokens") {
        Permission::TokensManage
    } else if route.starts_with("/workspace/members") || route.starts_with("/workspace/invitations")
    {
        Permission::MembersManage
    } else if route.starts_with("/workspace/roles") {
        Permission::RolesManage
    } else if route.starts_with("/workspace/navigation") {
        Permission::WorkspaceNavigationManage
    } else if route == "/workspace/identity-provider" {
        Permission::WorkspaceManage
    } else if route.starts_with("/workspace/agent-settings") {
        Permission::AgentsManage
    } else if route == "/workspace/tag-colors" {
        Permission::TagColorsManage
    } else if method == Method::DELETE {
        Permission::RecordsDelete
    } else {
        Permission::RecordsWrite
    }
}

fn audit_context(method: &Method, route: &str, path: &str) -> (Value, Value) {
    let permission = audit_permission(method, route);
    let segments: Vec<_> = path
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect();
    let identifier = segments
        .iter()
        .find_map(|segment| segment.parse::<Uuid>().ok());
    let target_type = if route.contains("/extension-registries") {
        "extension_registry"
    } else if route.starts_with("/extensions") {
        "extension"
    } else if route.contains("/workflows") {
        "workflow"
    } else if route.contains("/blueprints") {
        "blueprint"
    } else if route.starts_with("/lexicon") {
        "lexicon"
    } else if route.contains("/contexts") || route.contains("/publication-channels") {
        "context"
    } else if route.contains("/records") {
        "record"
    } else if route.contains("/workspace") {
        "workspace"
    } else {
        "catalog"
    };
    (
        json!({ "permission": permission }),
        match identifier {
            Some(id) => json!({ "type": target_type, "id": id }),
            None => json!({ "type": target_type, "route": route }),
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
    fn mutation_audit_permissions_match_workspace_management_routes() {
        assert_eq!(
            audit_permission(&Method::POST, "/personal-access-tokens"),
            Permission::TokensManage
        );
        assert_eq!(
            audit_permission(&Method::PATCH, "/workspace/members/{id}"),
            Permission::MembersManage
        );
        assert_eq!(
            audit_permission(&Method::DELETE, "/workspace/roles/{id}"),
            Permission::RolesManage
        );
        assert_eq!(
            audit_permission(&Method::PUT, "/workspace/agent-settings"),
            Permission::AgentsManage
        );
        assert_eq!(
            audit_permission(&Method::PATCH, "/workspace/tag-colors"),
            Permission::TagColorsManage
        );
        assert_eq!(
            audit_permission(
                &Method::POST,
                "/extensions/{extension_id}/{contribution_id}/command"
            ),
            Permission::RecordsWrite
        );
    }

    #[test]
    fn workflow_mutations_audit_workflow_management_permission() {
        assert_eq!(
            audit_permission(&Method::POST, "/workflows/{workflow_id}/disable"),
            Permission::WorkflowsManage
        );
        let (_, target) = audit_context(
            &Method::POST,
            "/workflows/{workflow_id}/disable",
            "/workflows/00000000-0000-4000-8000-000000000001/disable",
        );
        assert_eq!(target["type"], "workflow");
    }

    #[test]
    fn redacts_secrets_recursively_without_losing_safe_metadata() {
        assert_eq!(
            redact_metadata(json!({
                "password": "not retained",
                "nested": { "accessToken": "also not retained", "target": "record" },
                "items": [{ "api_key": "nope" }],
            })),
            json!({
                "password": "[REDACTED]",
                "nested": { "accessToken": "[REDACTED]", "target": "record" },
                "items": [{ "api_key": "[REDACTED]" }],
            })
        );
    }
}
