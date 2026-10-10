import type { QueryClient } from '@tanstack/react-query';
import { recordQueryKeys } from './queryKeys';

export const invalidateRecordSearches = (client: QueryClient) =>
  client.invalidateQueries({ queryKey: recordQueryKeys.searches() });

/** Publication changes also affect server-side search ordering. */
export const invalidateRecordPublications = (client: QueryClient, id: string) =>
  Promise.all([
    client.invalidateQueries({ queryKey: recordQueryKeys.publication(id) }),
    invalidateRecordSearches(client),
  ]);

const recordProjections = (id: string) => [
  recordQueryKeys.form(id),
  recordQueryKeys.preview(id),
  recordQueryKeys.resolvedPreviews(id),
  recordQueryKeys.changes(id),
  recordQueryKeys.recordControls(id),
  recordQueryKeys.publication(id),
  recordQueryKeys.migrationPreview(id),
  recordQueryKeys.hierarchies(id),
  recordQueryKeys.incomingRelationshipResults(id),
];

/** Every projection of a record must be refreshed after its values change. */
export const invalidateRecord = (client: QueryClient, id: string) =>
  Promise.all([
    ...recordProjections(id).map((queryKey) =>
      client.invalidateQueries({ queryKey }),
    ),
    // Labels are looked up in batches that may include other records.
    client.invalidateQueries({
      queryKey: recordQueryKeys.allLabels(),
      predicate: (query) => query.queryKey.includes(id),
    }),
    invalidateRecordSearches(client),
  ]);

/** Do not refetch a deleted record; discard its cached projections instead. */
export const removeRecord = (client: QueryClient, id: string) => {
  for (const queryKey of recordProjections(id)) {
    client.removeQueries({ queryKey });
  }
  return invalidateRecordSearches(client);
};
