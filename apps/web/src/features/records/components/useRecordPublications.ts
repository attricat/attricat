import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { publishRecordAllChannels } from '../api';
import {
  recordPublicationOptions,
  recordPublicationReadinessOptions,
} from '../queryOptions';
import { invalidateRecordPublications } from '../invalidateRecord';
import { readinessForContext } from '../checkViolations';
import { usePublicationMutations } from '../usePublicationMutations';

/**
 * Loads a record's channel publications and exposes publishing actions.
 * With `checkReadiness`, it also evaluates each enabled channel's checks.
 */
export const useRecordPublications = (
  recordId: string,
  contextId: string | null,
  checkReadiness = false,
) => {
  const client = useQueryClient();
  const publications = useQuery(recordPublicationOptions(recordId));
  const readiness = useQuery({
    ...recordPublicationReadinessOptions(recordId),
    enabled: checkReadiness,
  });
  const { publish, unpublish } = usePublicationMutations();
  const publishAll = useMutation({
    mutationFn: () => publishRecordAllChannels(recordId),
    onSuccess: () => invalidateRecordPublications(client, recordId),
  });
  return {
    error: publish.error ?? publishAll.error ?? unpublish.error,
    isPending: publish.isPending || publishAll.isPending || unpublish.isPending,
    publication:
      contextId === null
        ? undefined
        : publications.data?.find((item) => item.context_id === contextId),
    readiness: readinessForContext(readiness.data, contextId),
    notReadyChannels: (readiness.data ?? []).filter((item) => !item.ready),
    publish: () => {
      if (contextId) publish.mutate({ recordId, contextId });
    },
    publishAll: () => publishAll.mutate(),
    unpublish: () => {
      if (contextId) unpublish.mutate({ recordId, contextId });
    },
  };
};

export type RecordPublications = ReturnType<typeof useRecordPublications>;
