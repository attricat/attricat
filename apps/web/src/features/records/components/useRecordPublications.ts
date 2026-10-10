import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import {
  publishRecord,
  publishRecordAllChannels,
  unpublishRecord,
} from '../api';
import {
  recordPublicationOptions,
  recordPublicationReadinessOptions,
} from '../queryOptions';
import { invalidateRecordPublications } from '../invalidateRecord';
import { readinessForContext } from '../checkViolations';

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
  const invalidatePublications = () =>
    invalidateRecordPublications(client, recordId);
  const publish = useMutation({
    mutationFn: (targetContextId: string) =>
      publishRecord(recordId, targetContextId),
    onSuccess: invalidatePublications,
  });
  const publishAll = useMutation({
    mutationFn: () => publishRecordAllChannels(recordId),
    onSuccess: invalidatePublications,
  });
  const unpublish = useMutation({
    mutationFn: (targetContextId: string) =>
      unpublishRecord(recordId, targetContextId),
    onSuccess: invalidatePublications,
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
      if (contextId) publish.mutate(contextId);
    },
    publishAll: () => publishAll.mutate(),
    unpublish: () => {
      if (contextId) unpublish.mutate(contextId);
    },
  };
};
