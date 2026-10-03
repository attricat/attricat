import { queryOptions } from '@tanstack/react-query';
import {
  getEntityPublicationReadiness,
  getEntityPublications,
  getEntityStatusTransitions,
} from './api';
import { entityQueryKeys } from './queryKeys';

// Revisited virtual rows reuse their result; mutations explicitly invalidate it.
const publicationStaleTimeMs = 60_000;

export const entityPublicationOptions = (entityId: string) =>
  queryOptions({
    queryKey: entityQueryKeys.publication(entityId),
    queryFn: ({ signal }) => getEntityPublications(entityId, signal),
    staleTime: publicationStaleTimeMs,
  });

/** Readiness is evaluated live, so it is refetched whenever it is observed. */
export const entityPublicationReadinessOptions = (entityId: string) =>
  queryOptions({
    queryKey: entityQueryKeys.publicationReadiness(entityId),
    queryFn: ({ signal }) => getEntityPublicationReadiness(entityId, signal),
  });

export const entityStatusTransitionOptions = (
  entityId: string,
  contextId: string,
) =>
  queryOptions({
    queryKey: entityQueryKeys.statusTransitions(entityId, contextId),
    queryFn: ({ signal }) =>
      getEntityStatusTransitions(entityId, contextId, signal),
  });
