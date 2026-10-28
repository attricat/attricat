import type { QueryClient } from '@tanstack/react-query';
import { recordQueryKeys } from '../records/queryKeys';

/**
 * Refreshes what the server rewrites beside a record's findings: its
 * `attricat.needs_attention` tag and data quality on the record's own views,
 * and the searches and system-tag facets that filter or show them. Searches
 * are only marked stale, as after a record edit, so results behind an open
 * record panel keep their rows until shown again.
 */
export const refreshRecordRuleState = (client: QueryClient, recordId: string) =>
  Promise.all([
    ...[
      recordQueryKeys.form(recordId),
      recordQueryKeys.resolvedPreviews(recordId),
    ].map((queryKey) => client.invalidateQueries({ queryKey })),
    // System-tag facets live under searches.
    client.invalidateQueries({
      queryKey: recordQueryKeys.searches(),
      refetchType: 'none',
    }),
  ]);
