export const extensionRunQueryKeys = {
  all: () => ['extension-runs'] as const,
  list: () => [...extensionRunQueryKeys.all(), 'list'] as const,
  detail: (runId: string) =>
    [...extensionRunQueryKeys.all(), 'detail', runId] as const,
} as const;
