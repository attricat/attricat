import type { ConversationScope } from './constants';

export const agentQueryKeys = {
  all: () => ['agents'] as const,
  conversations: () => ['agents', 'conversations'] as const,
  conversationSearch: (query: string, scope: ConversationScope) =>
    ['agents', 'conversations', 'search', scope, query] as const,
  recordConversations: (recordId: string) =>
    ['agents', 'conversations', 'record', recordId] as const,
  conversation: (id: string) => ['agents', 'conversations', id] as const,
  messages: (id: string) =>
    ['agents', 'conversations', id, 'messages'] as const,
  runs: (id: string) => ['agents', 'conversations', id, 'runs'] as const,
  approvals: (conversationId?: string) =>
    ['agents', 'approvals', conversationId ?? 'all'] as const,
};
