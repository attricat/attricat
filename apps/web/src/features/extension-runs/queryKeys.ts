export const extensionRunQueryKeys = {
  all: () => ['extension-runs'] as const,
  list: (extensionId?: string) =>
    [...extensionRunQueryKeys.all(), 'list', extensionId ?? null] as const,
  detail: (runId: string) =>
    [...extensionRunQueryKeys.all(), 'detail', runId] as const,
} as const;
