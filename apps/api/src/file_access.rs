//! Replaceable authorization seam for file operations.
//!
//! The HTTP layer invokes this policy for every file read and write before it
//! touches object storage. Implementations must not reveal authorization
//! details to callers.

use async_trait::async_trait;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileAccessOperation {
    Upload {
        entity_id: Uuid,
    },
    ConversationUpload {
        conversation_id: Uuid,
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

#[async_trait]
impl FileAccessPolicy for AllowFileAccess {
    async fn authorize(&self, _: FileAccessOperation) -> FileAccessDecision {
        FileAccessDecision::Allow
    }
}
