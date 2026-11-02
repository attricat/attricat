import { recordQueryKeys } from '../records/queryKeys';

const fieldNotesSegment = 'extension-field-notes';

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
      fieldNotesSegment,
      contextId,
    ] as const,
  /** Under record searches, which refresh with the results they annotate. */
  searchFieldNotes: (
    recordIds: readonly string[],
    contextId: string | null,
    language: string,
  ) =>
    [
      ...recordQueryKeys.searches(),
      fieldNotesSegment,
      recordIds,
      contextId,
      language,
    ] as const,
};
