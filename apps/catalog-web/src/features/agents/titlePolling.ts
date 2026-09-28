import type { Conversation } from './schemas';

// Avoid polling indefinitely for a conversation whose provider is unavailable
// or which was created but never received a message.
export const isTitlePending = (conversation: Conversation) =>
  conversation.title_source === 'pending' &&
  Date.now() - new Date(conversation.updated_at).getTime() < 5 * 60_000;
