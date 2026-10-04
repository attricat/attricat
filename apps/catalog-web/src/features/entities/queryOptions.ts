import { queryOptions } from '@tanstack/react-query';
import {
  getEntityApprovals,
  getEntityPublicationReadiness,
  getEntityPublications,
  getEntityRetentionHolds,
  getStatusTransitions,
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

export const entityApprovalsOptions = (entityId: string) =>
  queryOptions({
    queryKey: entityQueryKeys.approvals(entityId),
    queryFn: ({ signal }) => getEntityApprovals(entityId, signal),
  });

export const entityRetentionHoldsOptions = (entityId: string) =>
  queryOptions({
    queryKey: entityQueryKeys.retentionHolds(entityId),
    queryFn: ({ signal }) => getEntityRetentionHolds(entityId, signal),
  });

/** The caller's access to declared status edges from the saved status. */
export const entityStatusTransitionsOptions = (
  entityId: string,
  contextId: string | null,
) =>
  queryOptions({
    queryKey: entityQueryKeys.statusTransitions(entityId, contextId),
    queryFn: ({ signal }) => getStatusTransitions(entityId, contextId, signal),
  });
