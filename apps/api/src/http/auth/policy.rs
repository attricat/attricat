use axum::http::Method;
use uuid::Uuid;

#[derive(Clone, Copy)]
pub(super) struct Policy {
    pub(super) permission: &'static str,
    pub(super) target: TargetKind,
}

#[derive(Clone, Copy)]
pub(super) enum TargetKind {
    None,
    BlueprintId,
    BlueprintCode,
    EntityId,
    FileRead,
    WorkspaceNavigation,
    ContextId,
    ContextCode,
    ContextList,
    PublicationChannelContext,
}

pub(super) fn policy(method: &Method, path: &str) -> Option<Policy> {
    let read = |target| Policy {
        permission: "entities.read",
        target,
    };
    let write = |target| Policy {
        permission: "entities.write",
        target,
    };
    let blueprint = if method == Method::GET {
        "blueprints.read"
    } else if path.ends_with("/publish") || path.ends_with("/safe-migration-batches") {
        "blueprints.publish"
    } else {
        "blueprints.write"
    };
    if path == "/solution-packs/inspect"
        || (method == Method::POST && path == "/solution-packs/plans")
        || (method == Method::GET && path == "/solution-packs/plans/{plan_id}")
        || (method == Method::POST && path == "/solution-packs/plans/{plan_id}/apply")
        || (method == Method::GET && path == "/solution-packs/applications")
        || (method == Method::GET && path == "/solution-packs/applications/{application_id}")
    {
        return Some(Policy {
            permission: "solution_packs.manage",
            target: TargetKind::None,
        });
    }
    if path == "/extension-registries/discover"
        || path.starts_with("/extension-registries/extensions/")
    {
        return Some(Policy {
            permission: "extensions.read",
            target: TargetKind::None,
        });
    }
    if path == "/extension-registries" || path.starts_with("/extension-registries/") {
        return Some(Policy {
            permission: "extensions.manage",
            target: TargetKind::None,
        });
    }
    if path == "/extensions/{extension_id}/{contribution_id}/command" {
        return Some(Policy {
            permission: "entities.write",
            target: TargetKind::None,
        });
    }
    if path == "/extensions/runtime"
        || path == "/extensions/{extension_id}/{contribution_id}/artifact"
        || path == "/extensions/{extension_id}/{contribution_id}/storage/{release_id}"
    {
        return Some(Policy {
            permission: "entities.read",
            target: TargetKind::None,
        });
    }
    if path == "/extensions"
        || path == "/extensions/sideload"
        || path.starts_with("/extensions/{extension_id}")
    {
        return Some(Policy {
            permission: if method == Method::GET {
                "extensions.read"
            } else {
                "extensions.manage"
            },
            target: TargetKind::None,
        });
    }
    if path == "/workflows"
        || path == "/workflows/validate"
        || path.starts_with("/workflows/")
        || path == "/workflow-runs"
        || path.starts_with("/workflow-runs/")
    {
        return Some(Policy {
            permission: if method == Method::GET {
                "workflows.read"
            } else {
                "workflows.manage"
            },
            target: TargetKind::None,
        });
    }
    if path.starts_with("/agent/") {
        return Some(Policy {
            permission: "agents.run",
            target: TargetKind::None,
        });
    }
    if path == "/workspace/invitations/accept" {
        return None;
    }
    if path.starts_with("/event-deliveries/") {
        return Some(Policy {
            permission: if method == Method::GET {
                "data_health.read"
            } else {
                "roles.manage"
            },
            target: TargetKind::None,
        });
    }
    if path == "/audit-events" {
        return Some(Policy {
            permission: "audit.read",
            target: TargetKind::None,
        });
    }
    if path == "/workspace/extension-layout" {
        return Some(Policy {
            permission: "extensions.manage",
            target: TargetKind::None,
        });
    }
    if path == "/workspace/extensions-mode" {
        return Some(Policy {
            permission: "extensions.manage",
            target: TargetKind::None,
        });
    }
    if path == "/workspace/assignable-roles" || path.starts_with("/workspace/grant-targets/") {
        return Some(Policy {
            permission: "members.manage",
            target: TargetKind::None,
        });
    }
    if path == "/workspace/navigation/sidebar" {
        return Some(Policy {
            permission: "entities.read",
            target: TargetKind::WorkspaceNavigation,
        });
    }
    if path == "/workspace/navigation" {
        return Some(Policy {
            permission: "workspace_navigation.manage",
            target: TargetKind::None,
        });
    }
    if path == "/workspace/token-permissions" {
        return Some(Policy {
            permission: "tokens.manage",
            target: TargetKind::None,
        });
    }
    if path == "/workspace/roles"
        || path == "/workspace/permissions"
        || path.starts_with("/workspace/roles/")
    {
        return Some(Policy {
            permission: "roles.manage",
            target: TargetKind::None,
        });
    }
    if path == "/workspace/users"
        || path == "/workspace/members"
        || path.starts_with("/workspace/members/")
        || path == "/workspace/invitations"
        || path.starts_with("/workspace/invitations/")
    {
        return Some(Policy {
            permission: "members.manage",
            target: TargetKind::None,
        });
    }
    if path == "/personal-access-tokens" || path.starts_with("/personal-access-tokens/") {
        return Some(Policy {
            permission: "tokens.manage",
            target: TargetKind::None,
        });
    }
    if path == "/metrics" || path.starts_with("/data-health/") {
        return Some(Policy {
            permission: "data_health.read",
            target: TargetKind::None,
        });
    }
    if path == "/blueprints/{blueprint_id}/versions/{version}/entity-publications"
        || path == "/blueprints/{blueprint_id}/versions/{version}/entity-publications/publish-all"
    {
        return Some(Policy {
            permission: "entities.publish",
            target: TargetKind::BlueprintId,
        });
    }
    if path.starts_with("/blueprints/by-code/{code}") {
        return Some(Policy {
            permission: blueprint,
            target: TargetKind::BlueprintCode,
        });
    }
    if path.starts_with("/blueprints/{blueprint_id}") {
        return Some(Policy {
            permission: blueprint,
            target: TargetKind::BlueprintId,
        });
    }
    if path == "/blueprints" || path == "/blueprints/catalogue" {
        return Some(Policy {
            permission: blueprint,
            target: TargetKind::None,
        });
    }
    if path == "/publication-channels" {
        return Some(Policy {
            permission: "contexts.read",
            target: TargetKind::None,
        });
    }
    if path == "/publication-channels/{context_id}" {
        return Some(Policy {
            permission: "contexts.write",
            target: TargetKind::PublicationChannelContext,
        });
    }
    if path == "/contexts/{code}" {
        return Some(Policy {
            permission: if method == Method::GET {
                "contexts.read"
            } else {
                "contexts.write"
            },
            target: TargetKind::ContextCode,
        });
    }
    if path == "/contexts/id/{id}" {
        return Some(Policy {
            permission: if method == Method::GET {
                "contexts.read"
            } else {
                "contexts.write"
            },
            target: TargetKind::ContextId,
        });
    }
    if path == "/contexts" {
        return Some(Policy {
            permission: if method == Method::GET {
                "contexts.read"
            } else {
                "contexts.write"
            },
            target: if method == Method::GET {
                TargetKind::ContextList
            } else {
                TargetKind::None
            },
        });
    }
    if path.starts_with("/files/{file_id}") {
        return Some(read(TargetKind::FileRead));
    }
    if path.starts_with("/v1/entities/{entity_id}/publications") && method != Method::GET {
        return Some(Policy {
            permission: "entities.publish",
            target: TargetKind::EntityId,
        });
    }
    if path.starts_with("/v1/entities/{entity_id}") || path.starts_with("/entities/{entity_id}") {
        return Some(if method == Method::DELETE {
            Policy {
                permission: "entities.delete",
                target: TargetKind::EntityId,
            }
        } else if method == Method::GET {
            read(TargetKind::EntityId)
        } else {
            write(TargetKind::EntityId)
        });
    }
    if path == "/v1/entities" {
        return Some(write(TargetKind::None));
    }
    if path == "/v1/entities/search"
        || path == "/v1/entities/facets/relationship-tree/children"
        || path == "/entities"
    {
        return Some(read(TargetKind::None));
    }
    None
}

pub(super) fn target(path: &str, kind: TargetKind) -> (Option<Uuid>, Option<String>) {
    let segments: Vec<_> = path
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect();
    match kind {
        TargetKind::None => (None, None),
        TargetKind::BlueprintId => (segments.get(1).and_then(|value| value.parse().ok()), None),
        TargetKind::BlueprintCode => (None, segments.get(2).map(|value| (*value).to_owned())),
        TargetKind::FileRead | TargetKind::WorkspaceNavigation => (None, None),
        TargetKind::EntityId => {
            let index = if segments.first() == Some(&"v1") {
                2
            } else {
                1
            };
            (
                segments.get(index).and_then(|value| value.parse().ok()),
                None,
            )
        }
        TargetKind::ContextId => (segments.get(2).and_then(|value| value.parse().ok()), None),
        TargetKind::PublicationChannelContext => {
            (segments.get(1).and_then(|value| value.parse().ok()), None)
        }
        TargetKind::ContextCode => (None, segments.get(1).map(|value| (*value).to_owned())),
        TargetKind::ContextList => (None, Some("__context_list__".to_owned())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workflow_routes_require_read_or_manage_permissions() {
        assert_eq!(
            policy(&Method::GET, "/workflows").unwrap().permission,
            "workflows.read"
        );
        assert_eq!(
            policy(&Method::GET, "/workflows/{workflow_id}/versions/1")
                .unwrap()
                .permission,
            "workflows.read"
        );
        assert_eq!(
            policy(&Method::POST, "/workflows/validate")
                .unwrap()
                .permission,
            "workflows.manage"
        );
        assert_eq!(
            policy(&Method::POST, "/workflows/{workflow_id}/disable")
                .unwrap()
                .permission,
            "workflows.manage"
        );
    }

    #[test]
    fn publication_channel_routes_target_their_context() {
        let context_id = Uuid::new_v4();
        let route = "/publication-channels/{context_id}";
        let policy = policy(&Method::PUT, route).unwrap();
        assert_eq!(policy.permission, "contexts.write");
        assert!(matches!(
            policy.target,
            TargetKind::PublicationChannelContext
        ));
        assert_eq!(
            target(
                &format!("/publication-channels/{context_id}"),
                policy.target
            ),
            (Some(context_id), None)
        );
    }

    #[test]
    fn blueprint_entity_publication_routes_require_publish_permission() {
        assert_eq!(
            policy(
                &Method::POST,
                "/blueprints/{blueprint_id}/versions/{version}/entity-publications"
            )
            .unwrap()
            .permission,
            "entities.publish"
        );
        assert_eq!(
            policy(
                &Method::POST,
                "/blueprints/{blueprint_id}/versions/{version}/entity-publications/publish-all"
            )
            .unwrap()
            .permission,
            "entities.publish"
        );
    }

    #[test]
    fn entity_publication_routes_require_publish_permission() {
        assert_eq!(
            policy(&Method::POST, "/v1/entities/{entity_id}/publications")
                .unwrap()
                .permission,
            "entities.publish"
        );
        assert_eq!(
            policy(
                &Method::POST,
                "/v1/entities/{entity_id}/publications/publish-all"
            )
            .unwrap()
            .permission,
            "entities.publish"
        );
    }

    #[test]
    fn solution_pack_inspection_requires_manage_permission() {
        for (method, path) in [
            (Method::POST, "/solution-packs/inspect"),
            (Method::POST, "/solution-packs/plans"),
            (Method::GET, "/solution-packs/plans/{plan_id}"),
            (Method::POST, "/solution-packs/plans/{plan_id}/apply"),
            (Method::GET, "/solution-packs/applications"),
            (Method::GET, "/solution-packs/applications/{application_id}"),
        ] {
            assert_eq!(
                policy(&method, path).unwrap().permission,
                "solution_packs.manage"
            );
        }
        assert!(policy(&Method::POST, "/solution-packs/apply").is_none());
        assert!(policy(&Method::GET, "/solution-packs/future").is_none());
    }

    #[test]
    fn extension_management_routes_require_extension_permissions() {
        assert_eq!(
            policy(&Method::GET, "/extensions").unwrap().permission,
            "extensions.read"
        );
        assert_eq!(
            policy(&Method::POST, "/extensions").unwrap().permission,
            "extensions.manage"
        );
        assert_eq!(
            policy(&Method::POST, "/extensions/sideload")
                .unwrap()
                .permission,
            "extensions.manage"
        );
        for method in [Method::GET, Method::PUT] {
            assert_eq!(
                policy(&method, "/workspace/extension-layout")
                    .unwrap()
                    .permission,
                "extensions.manage"
            );
        }
        assert_eq!(
            policy(&Method::POST, "/extensions/{extension_id}/enable")
                .unwrap()
                .permission,
            "extensions.manage"
        );
        assert_eq!(
            policy(
                &Method::GET,
                "/extensions/{extension_id}/{contribution_id}/artifact"
            )
            .unwrap()
            .permission,
            "entities.read"
        );
        assert_eq!(
            policy(
                &Method::POST,
                "/extensions/{extension_id}/{contribution_id}/storage/{release_id}"
            )
            .unwrap()
            .permission,
            "entities.read"
        );
        assert_eq!(
            policy(
                &Method::POST,
                "/extensions/{extension_id}/{contribution_id}/command"
            )
            .unwrap()
            .permission,
            "entities.write"
        );
    }
}
