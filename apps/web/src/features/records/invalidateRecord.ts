import type { QueryClient } from '@tanstack/react-query';
import { ruleQueryKeys } from '../rules/queryKeys';
import { watchRecordRuleChecks } from '../rules/recordRuleChecks';
import { recordQueryKeys } from './queryKeys';

export const invalidateRecordSearches = (client: QueryClient) =>
  client.invalidateQueries({ queryKey: recordQueryKeys.searches() });

/**
 * Refetches searches on screen that a record edit marked stale, such as the
 * Explorer results once its record panel closes.
 */
export const refreshStaleRecordSearches = (client: QueryClient) =>
  client.refetchQueries({
    queryKey: recordQueryKeys.searches(),
    predicate: (query) => query.state.isInvalidated,
    type: 'active',
  });

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

/**
 * Every projection of a record must be refreshed after its values change.
 * Searches are only marked stale: results on screen, such as the Explorer
 * behind its record panel, keep their rows while the record is edited and
 * refetch when shown again or when the panel closes. The change also re-checks
 * the record's rules, so its rule runs are watched until they finish.
 */
export const invalidateRecord = (client: QueryClient, id: string) => {
  watchRecordRuleChecks(id);
  return Promise.all([
    ...recordProjections(id).map((queryKey) =>
      client.invalidateQueries({ queryKey }),
    ),
    client.invalidateQueries({ queryKey: ruleQueryKeys.recordRuns(id) }),
    // Labels are looked up in batches that may include other records.
    client.invalidateQueries({
      queryKey: recordQueryKeys.allLabels(),
      predicate: (query) => query.queryKey.includes(id),
    }),
    client.invalidateQueries({
      queryKey: recordQueryKeys.searches(),
      refetchType: 'none',
    }),
  ]);
};

/** Do not refetch a deleted record; discard its cached projections instead. */
export const removeRecord = (client: QueryClient, id: string) => {
  for (const queryKey of recordProjections(id)) {
    client.removeQueries({ queryKey });
  }
  return invalidateRecordSearches(client);
};
