import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import {
  publishEntity,
  publishEntityAllChannels,
  unpublishEntity,
} from '../api';
import {
  entityPublicationOptions,
  entityPublicationReadinessOptions,
} from '../queryOptions';
import { invalidateEntityPublications } from '../invalidateEntity';
import { readinessForContext } from '../checkViolations';

/**
 * Loads an entity's channel publications and exposes publishing actions.
 * With `checkReadiness`, it also evaluates each enabled channel's checks.
 */
export const useEntityPublications = (
  entityId: string,
  contextId: string | null,
  checkReadiness = false,
) => {
  const client = useQueryClient();
  const publications = useQuery(entityPublicationOptions(entityId));
  const readiness = useQuery({
    ...entityPublicationReadinessOptions(entityId),
    enabled: checkReadiness,
  });
  const invalidatePublications = () =>
    invalidateEntityPublications(client, entityId);
  const publish = useMutation({
    mutationFn: (targetContextId: string) =>
      publishEntity(entityId, targetContextId),
    onSuccess: invalidatePublications,
  });
  const publishAll = useMutation({
    mutationFn: () => publishEntityAllChannels(entityId),
    onSuccess: invalidatePublications,
  });
  const unpublish = useMutation({
    mutationFn: (targetContextId: string) =>
      unpublishEntity(entityId, targetContextId),
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
