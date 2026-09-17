export const blueprintQueryKeys = {
  catalogue: () => ['blueprint-catalogue'] as const,
  revisions: (id: string) => ['blueprint-revisions', id] as const,
  migrationBatches: (id: string) =>
    ['blueprint-migration-batches', id] as const,
  revision: (id: string, version: number) =>
    ['blueprint-revision', id, version] as const,
} as const;
