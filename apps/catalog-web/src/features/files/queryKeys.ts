export const fileQueryKeys = {
  all: () => ['files'] as const,
  metadata: (id: string) => [...fileQueryKeys.all(), 'metadata', id] as const,
};
