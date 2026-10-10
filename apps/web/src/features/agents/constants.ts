export const agentRoutes = {
  list: '/agents',
  create: '/agents/new',
  detail: '/agents/$conversationId',
} as const;

export const messageRoles = {
  user: 'user',
  assistant: 'assistant',
  tool: 'tool',
} as const;

export const activeRunStatuses: readonly string[] = [
  'queued',
  'running',
  'awaiting_approval',
];
export const thinkingRunStatuses: readonly string[] = ['queued', 'running'];

export const runUpdateEvents = [
  'status',
  'message',
  'tool_call',
  'approval_required',
  'error',
  'terminal',
] as const;

export const pendingTitleSource = 'pending';

export const titlePollIntervalMs = 3_000;
export const titlePendingTimeoutMs = 5 * 60_000;
export const conversationPollIntervalMs = 10_000;
export const conversationReconcileIntervalMs = 30_000;
export const conversationIdlePollIntervalMs = 60_000;
export const conversationEventBatchDelayMs = 250;

export const maximumTitleLength = 72;
export const maximumTitleBytes = 512;
export const maximumSearchLength = 120;

export const composerMaxRows = 8;
export const composerMinRows = 1;
export const newConversationMinRows = 3;
export const newConversationMaxWidth = 760;
export const avatarSize = 30;
export const progressSize = 20;
export const loadingProgressSize = 18;
export const jsonIndent = 2;
export const composerBorderRadius = 3;
export const messageBubbleRadius = 3;
export const userMessageMaxWidth = { xs: '100%', sm: '80%' } as const;
export const transcriptEndScrollMargin = { md: '8rem', xs: '10rem' } as const;
export const conversationMinHeight = { md: 'calc(100dvh - 48px)' } as const;

export const thinkingDotCount = 3;
export const thinkingDotDelaysSeconds = [0, 0.16, 0.32] as const;
export const thinkingAnimationName = 'agent-thinking';
export const thinkingAnimationDuration = '1.4s';

export const uuidPattern =
  '[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}';
