export const blueprintQueryKeys = {
  catalogue: () => ['blueprint-catalogue'] as const,
  revisions: (id: string) => ['blueprint-revisions', id] as const,
  migrationBatches: (id: string) =>
    ['blueprint-migration-batches', id] as const,
  revision: (id: string, version: number) =>
    ['blueprint-revision', id, version] as const,
  safeMigrationImpact: (id: string, version: number) =>
    [
      ...blueprintQueryKeys.revision(id, version),
      'safe-migration-impact',
    ] as const,
} as const;
