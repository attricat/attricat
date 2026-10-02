import { useNavigate } from '@tanstack/react-router';
import { useMutation, useQueries, useQueryClient } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import {
  duplicateEntity,
  publishEntity,
  unpublishEntity,
  type EntityItem,
  type EntityPublicationStatus,
} from '../entities/api';
import { entityQueryKeys } from '../entities/queryKeys';
import { entityPublicationOptions } from '../entities/queryOptions';
import {
  invalidateEntityPublications,
  invalidateEntitySearches,
} from '../entities/invalidateEntity';

/** Observe only visible rows (plus an open action menu), never all loaded pages. */
export const useEntityPublicationActions = (
  items: EntityItem[],
  publicationContextId: string | undefined,
) => {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const navigate = useNavigate();
  const publicationQueries = useQueries({
    queries: items.map((entity) => ({
      ...entityPublicationOptions(entity.id),
      enabled: Boolean(publicationContextId),
    })),
  });
  const publicationsByEntityId = new Map(
    items.map((entity, index) => [
      entity.id,
      publicationQueries[index]?.data?.find(
        (publication) => publication.context_id === publicationContextId,
      ),
    ]),
  );
  const requirePublicationContext = () => {
    if (!publicationContextId)
      throw new Error(t('explorer.publicationContextUnavailable'));
    return publicationContextId;
  };
  const updatePublication = (
    entityId: string,
    publication: EntityPublicationStatus,
  ) =>
    queryClient.setQueryData<EntityPublicationStatus[]>(
      entityQueryKeys.publication(entityId),
      (current) => [
        ...(current ?? []).filter(
          (item) => item.context_id !== publication.context_id,
        ),
        publication,
      ],
    );
  const invalidatePublication = (entityId: string) =>
    invalidateEntityPublications(queryClient, entityId);
  const publish = useMutation({
    mutationFn: (entityId: string) =>
      publishEntity(entityId, requirePublicationContext()),
    onSuccess: (publication, entityId) => {
      updatePublication(entityId, publication);
      void invalidatePublication(entityId);
    },
  });
  const unpublish = useMutation({
    mutationFn: (entityId: string) =>
      unpublishEntity(entityId, requirePublicationContext()),
    onSuccess: (_, entityId) => void invalidatePublication(entityId),
  });
  const duplicate = useMutation({
    mutationFn: (entityId: string) => duplicateEntity(entityId),
    onSuccess: (entity) => {
      void invalidateEntitySearches(queryClient);
      void navigate({
        params: { entityId: entity.id },
        to: '/entities/$entityId/edit',
      });
    },
  });

  return {
    duplicate,
    error: publish.error ?? unpublish.error ?? duplicate.error,
    publicationsByEntityId,
    publish,
    unpublish,
  };
};
