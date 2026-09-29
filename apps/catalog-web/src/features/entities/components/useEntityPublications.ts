import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import {
  getEntityPublications,
  publishEntity,
  publishEntityAllChannels,
  unpublishEntity,
} from '../api';
import { entityQueryKeys } from '../queryKeys';

/** Loads an entity's channel publications and exposes publishing actions. */
export const useEntityPublications = (
  entityId: string,
  contextId: string | null,
) => {
  const client = useQueryClient();
  const publications = useQuery({
    queryKey: entityQueryKeys.publication(entityId),
    queryFn: () => getEntityPublications(entityId),
  });
  const invalidatePublications = () =>
    client.invalidateQueries({
      queryKey: entityQueryKeys.publication(entityId),
    });
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
    publish: () => {
      if (contextId) publish.mutate(contextId);
    },
    publishAll: () => publishAll.mutate(),
    unpublish: () => {
      if (contextId) unpublish.mutate(contextId);
    },
  };
};
