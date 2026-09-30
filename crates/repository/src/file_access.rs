//! Replaceable authorization seam for file operations.
//!
//! The HTTP layer invokes this policy for every file read and write before it
//! touches object storage. Implementations must not reveal authorization
//! details to callers.

use async_trait::async_trait;
use uuid::Uuid;

use crate::repository::{CatalogRepository, RepositoryError};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileAccessOperation {
    Upload {
        entity_id: Uuid,
    },
    ConversationUpload {
        conversation_id: Uuid,
    },
    /// The authenticated member replacing their own avatar.
    AvatarUpload {
        user_id: Uuid,
    },
    /// Reading the processed variant of a workspace member's avatar.
    AvatarRead {
        file_id: Uuid,
    },
    ReadMetadata {
        file_id: Uuid,
        entity_id: Uuid,
        blueprint_id: Uuid,
    },
    DownloadOriginal {
        file_id: Uuid,
        entity_id: Uuid,
        blueprint_id: Uuid,
    },
    DownloadVariant {
        file_id: Uuid,
        entity_id: Uuid,
        blueprint_id: Uuid,
    },
    /// A provider-visible file read requested by an approved user through an
    /// agent run.
    AgentRead {
        file_id: Uuid,
        entity_id: Uuid,
        blueprint_id: Uuid,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileAccessDecision {
    Allow,
    Deny,
}

#[async_trait]
pub trait FileAccessPolicy: Send + Sync {
    async fn authorize(&self, operation: FileAccessOperation) -> FileAccessDecision;
}

/// Default policy. Request authentication and workspace scoping remain enforced
/// by the normal HTTP middleware and repository; deployments can replace this
/// policy with resource-level authorization.
pub struct AllowFileAccess;

/// Applies the same repository grant and deployment-specific file policy used
/// by HTTP downloads to every caller that exposes file content.
pub async fn authorize_file_read(
    repository: &CatalogRepository,
    policy: &dyn FileAccessPolicy,
    principal: Uuid,
    workspace: Uuid,
    file_id: Uuid,
    operation: impl Fn(Uuid, Uuid, Uuid) -> FileAccessOperation,
) -> Result<bool, RepositoryError> {
    for target in repository.file_read_targets(file_id).await? {
        if repository
            .is_authorized(
                principal,
                workspace,
                "entities.read",
                Some(target.entity_id),
                None,
            )
            .await?
            && policy
                .authorize(operation(file_id, target.entity_id, target.blueprint_id))
                .await
                == FileAccessDecision::Allow
        {
            return Ok(true);
        }
    }
    Ok(false)
}

#[async_trait]
impl FileAccessPolicy for AllowFileAccess {
    async fn authorize(&self, _: FileAccessOperation) -> FileAccessDecision {
        FileAccessDecision::Allow
    }
}
