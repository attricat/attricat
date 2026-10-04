//! Workspace members, role grants and per-user clients in the bootstrap
//! workspace, inserted directly so tests can act as narrowly scoped users.

use reqwest::{
    Client,
    header::{HeaderMap, HeaderValue},
};
use sqlx::PgPool;
use uuid::Uuid;

use super::{BOOTSTRAP_WORKSPACE_ID, bootstrap_workspace_id};

pub const OWNER_ROLE_ID: Uuid = Uuid::from_u128(0x00000000_0000_4000_8000_000000000101);
pub const EDITOR_ROLE_ID: Uuid = Uuid::from_u128(0x00000000_0000_4000_8000_000000000103);
pub const VIEWER_ROLE_ID: Uuid = Uuid::from_u128(0x00000000_0000_4000_8000_000000000104);

/// Where a role grant applies.
#[derive(Clone, Copy, Debug)]
pub enum GrantScope {
    Workspace,
    Entity(Uuid),
}

/// A client that authenticates as `user` in the bootstrap workspace through
/// the trusted test headers.
pub fn client_for(user: Uuid) -> Client {
    let mut headers = HeaderMap::new();
    headers.insert(
        "x-catalog-user-id",
        HeaderValue::from_str(&user.to_string()).unwrap(),
    );
    headers.insert(
        "x-catalog-workspace-id",
        HeaderValue::from_static(BOOTSTRAP_WORKSPACE_ID),
    );
    Client::builder().default_headers(headers).build().unwrap()
}

/// A new user with an active bootstrap-workspace membership and no grants.
/// Returns `(user_id, membership_id)`.
pub async fn add_workspace_user(pool: &PgPool) -> (Uuid, Uuid) {
    let (user, membership) = (Uuid::new_v4(), Uuid::new_v4());
    sqlx::query("INSERT INTO users (id, email) VALUES ($1, $2)")
        .bind(user)
        .bind(format!("{user}@example.test"))
        .execute(pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO workspace_memberships (id, workspace_id, user_id) VALUES ($1, $2, $3)",
    )
    .bind(membership)
    .bind(bootstrap_workspace_id())
    .bind(user)
    .execute(pool)
    .await
    .unwrap();
    (user, membership)
}

/// Grants `role_id` to a membership and returns the grant id.
pub async fn grant_role(pool: &PgPool, membership: Uuid, role_id: Uuid, scope: GrantScope) -> Uuid {
    let grant = Uuid::new_v4();
    let (scope_type, target) = match scope {
        GrantScope::Workspace => ("workspace", bootstrap_workspace_id()),
        GrantScope::Entity(entity_id) => ("entity", entity_id),
    };
    sqlx::query("INSERT INTO role_grants (id, workspace_id, membership_id, role_id, scope_type, scope_target_id) VALUES ($1, $2, $3, $4, $5, $6)")
        .bind(grant)
        .bind(bootstrap_workspace_id())
        .bind(membership)
        .bind(role_id)
        .bind(scope_type)
        .bind(target)
        .execute(pool)
        .await
        .unwrap();
    grant
}

/// A workspace role named `code` holding `permissions`.
pub async fn create_role(pool: &PgPool, code: &str, permissions: &[&str]) -> Uuid {
    let role = Uuid::new_v4();
    sqlx::query("INSERT INTO roles (id, code, workspace_id) VALUES ($1, $2, $3)")
        .bind(role)
        .bind(code)
        .bind(bootstrap_workspace_id())
        .execute(pool)
        .await
        .unwrap();
    for permission in permissions {
        sqlx::query("INSERT INTO role_permissions (role_id, permission_code) VALUES ($1, $2)")
            .bind(role)
            .bind(permission)
            .execute(pool)
            .await
            .unwrap();
    }
    role
}

/// A new member holding `role_id` across the bootstrap workspace.
pub async fn member_with_role(pool: &PgPool, role_id: Uuid) -> Uuid {
    let (user, membership) = add_workspace_user(pool).await;
    grant_role(pool, membership, role_id, GrantScope::Workspace).await;
    user
}
