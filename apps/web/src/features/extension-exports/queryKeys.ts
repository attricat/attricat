const root = ['extension-exports'] as const;

export const extensionExportQueryKeys = {
  all: root,
  workspace: () => [...root, 'workspace'] as const,
  record: (recordId: string) => [...root, 'record', recordId] as const,
  extensionRoot: (extensionId: string) =>
    [...root, 'extension', extensionId] as const,
  extension: (
    extensionId: string,
    filter: { connection: string | null; status: string | null },
  ) =>
    [...extensionExportQueryKeys.extensionRoot(extensionId), filter] as const,
  /** The paged problem list; an infinite query cannot share the overview's entry. */
  extensionPages: (
    extensionId: string,
    filter: { connection: string | null; status: string | null },
  ) =>
    [
      ...extensionExportQueryKeys.extension(extensionId, filter),
      'pages',
    ] as const,
  connections: (extensionId: string) =>
    [...root, 'connections', extensionId] as const,
  /** Every mutation on one connection, so its row knows when one runs. */
  connectionMutations: (extensionId: string, connection: string) =>
    [...root, 'connections', extensionId, connection, 'mutations'] as const,
  budgets: (extensionId: string) => [...root, 'budgets', extensionId] as const,
  preview: (
    extensionId: string,
    exportId: string,
    recordId: string,
    connection: string | null,
  ) =>
    [...root, 'preview', extensionId, exportId, recordId, connection] as const,
};
