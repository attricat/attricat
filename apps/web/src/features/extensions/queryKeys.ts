import { recordQueryKeys } from '../records/queryKeys';

export const extensionQueryKeys = {
  all: ['extensions'] as const,
  runtimeRoot: () => ['extensions', 'runtime'] as const,
  runtime: (scope?: { blueprintId: string; blueprintVersion: number }) =>
    ['extensions', 'runtime', scope ?? null] as const,
  /**
   * Under the record's resolved previews: field notes describe its values, so
   * whatever refreshes those after an edit refreshes the notes too.
   */
  fieldNotes: (recordId: string, contextId: string | null) =>
    [
      ...recordQueryKeys.resolvedPreviews(recordId),
      'extension-field-notes',
      contextId,
    ] as const,
};
