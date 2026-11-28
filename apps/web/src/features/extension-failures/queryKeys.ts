const root = ['extension-failures'] as const;

export const extensionFailureQueryKeys = {
  all: root,
  runs: (extensionId: string) => [...root, extensionId, 'runs'] as const,
  deliveries: (extensionId: string) =>
    [...root, extensionId, 'deliveries'] as const,
};
