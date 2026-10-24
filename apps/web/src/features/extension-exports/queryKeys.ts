export const extensionExportQueryKeys = {
  all: ['extension-exports'] as const,
  workspace: () => ['extension-exports', 'workspace'] as const,
  record: (recordId: string) =>
    ['extension-exports', 'record', recordId] as const,
  extension: (
    extensionId: string,
    filter: { connection: string | null; status: string | null },
  ) => ['extension-exports', 'extension', extensionId, filter] as const,
  extensionRoot: (extensionId: string) =>
    ['extension-exports', 'extension', extensionId] as const,
  connections: (extensionId: string) =>
    ['extension-exports', 'connections', extensionId] as const,
  budgets: (extensionId: string) =>
    ['extension-exports', 'budgets', extensionId] as const,
};
