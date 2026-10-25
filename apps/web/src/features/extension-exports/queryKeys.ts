export const extensionExportQueryKeys = {
  all: ['extension-exports'] as const,
  workspace: () => ['extension-exports', 'workspace'] as const,
  record: (recordId: string) =>
    ['extension-exports', 'record', recordId] as const,
  extension: (
    extensionId: string,
    filter: { connection: string | null; status: string | null },
  ) => ['extension-exports', 'extension', extensionId, filter] as const,
  /** The paged problem list; an infinite query cannot share the overview's entry. */
  extensionPages: (
    extensionId: string,
    filter: { connection: string | null; status: string | null },
  ) =>
    [
      ...extensionExportQueryKeys.extension(extensionId, filter),
      'pages',
    ] as const,
  extensionRoot: (extensionId: string) =>
    ['extension-exports', 'extension', extensionId] as const,
  connections: (extensionId: string) =>
    ['extension-exports', 'connections', extensionId] as const,
  budgets: (extensionId: string) =>
    ['extension-exports', 'budgets', extensionId] as const,
  preview: (
    extensionId: string,
    exportId: string,
    recordId: string,
    connection: string | null,
  ) =>
    [
      'extension-exports',
      'preview',
      extensionId,
      exportId,
      recordId,
      connection,
    ] as const,
};
