import { useQueries, useQuery } from '@tanstack/react-query';
import type { RecordItem } from '../records/api';
import { readinessForContext } from '../records/checkViolations';
import {
  recordPublicationOptions,
  recordPublicationReadinessOptions,
} from '../records/queryOptions';
import { loadRecordPublications } from '../records/recordPublicationsLoader';
import { usePublicationMutations } from '../records/usePublicationMutations';

/**
 * Observe only visible rows (plus an open action menu), never all loaded pages.
 * Channel checks are evaluated only for `readinessRecordId` (the open menu).
 */
export const useRecordPublicationActions = (
  items: RecordItem[],
  publicationContextId: string | undefined,
  readinessRecordId?: string,
) => {
  const publicationQueries = useQueries({
    queries: items.map((record) => ({
      ...recordPublicationOptions(record.id),
      // Visible rows share one lookup instead of one request each.
      queryFn: () => loadRecordPublications(record.id),
      enabled: Boolean(publicationContextId),
    })),
  });
  const publicationsByRecordId = new Map(
    items.map((record, index) => [
      record.id,
      publicationQueries[index]?.data?.find(
        (publication) => publication.context_id === publicationContextId,
      ),
    ]),
  );
  const readinessQuery = useQuery({
    ...recordPublicationReadinessOptions(readinessRecordId),
    enabled: Boolean(publicationContextId),
  });
  const readiness = readinessRecordId
    ? readinessForContext(readinessQuery.data, publicationContextId)
    : undefined;
  const { publish, unpublish } = usePublicationMutations();

  const change =
    (mutation: typeof publish | typeof unpublish) => (recordId: string) => {
      if (publicationContextId)
        mutation.mutate({ recordId, contextId: publicationContextId });
    };
  return {
    error: publish.error ?? unpublish.error,
    publicationsByRecordId,
    publish: change(publish),
    publishing: publish.isPending,
    readiness,
    unpublish: change(unpublish),
    unpublishing: unpublish.isPending,
  };
};
