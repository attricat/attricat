import {
  pendingTitleSource,
  titlePendingTimeoutMs,
  titlePollIntervalMs,
} from './constants';
import type { Conversation } from './schemas';

// Avoid polling indefinitely for a conversation whose provider is unavailable
// or which was created but never received a message.
const isTitlePending = (conversation: Conversation) =>
  conversation.title_source === pendingTitleSource &&
  Date.now() - new Date(conversation.updated_at).getTime() <
    titlePendingTimeoutMs;

/** Poll interval while any of the conversations still waits for its title. */
export const titlePollInterval = (
  conversations: readonly Conversation[] | undefined,
) => (conversations?.some(isTitlePending) ? titlePollIntervalMs : false);
