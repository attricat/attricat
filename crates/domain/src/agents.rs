//! Agent domain and product safety invariants.

/// Agent API and runner bounds. These are intentionally fixed product safety
/// limits rather than deployment settings.
pub const MAX_CONVERSATION_TITLE_BYTES: usize = 512;
pub const MAX_AGENT_MODEL_BYTES: usize = 512;
pub const MAX_CONVERSATION_MESSAGE_BYTES: usize = 32 * 1024;
pub const MAX_CONVERSATION_ATTACHMENTS: usize = 16;
pub const MAX_INLINE_ATTACHMENT_BYTES: i64 = 5 * 1024 * 1024;
pub const MAX_TOOL_RESULT_BYTES: usize = 64 * 1024;
// Reasoning-capable providers can emit substantial hidden reasoning in their
// streamed chunks. Keep enough headroom for that wire data while retaining a
// finite response bound.
pub const MAX_PROVIDER_BODY_BYTES: usize = 256 * 1024;
pub const MAX_PROVIDER_FRAME_BUFFER_BYTES: usize = 64 * 1024;
pub const MAX_ASSISTANT_CONTENT_BYTES: usize = 32 * 1024;
pub const MAX_TOOL_CALL_ARGUMENT_BYTES: usize = 16 * 1024;
pub const MAX_TOOL_CALLS: usize = 32;
