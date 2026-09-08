export const agentQueryKeys = {
  all: () => ['agents'] as const,
  conversations: () => ['agents', 'conversations'] as const,
  conversation: (id: string) => ['agents', 'conversations', id] as const,
  messages: (id: string) =>
    ['agents', 'conversations', id, 'messages'] as const,
  runs: (id: string) => ['agents', 'conversations', id, 'runs'] as const,
  approvals: (conversationId?: string) =>
    ['agents', 'approvals', conversationId ?? 'all'] as const,
  schedules: (conversationId?: string) =>
    ['agents', 'schedules', conversationId ?? 'all'] as const,
};
