import { queryOptions } from '@tanstack/react-query';
import { getEntityPublications } from './api';
import { entityQueryKeys } from './queryKeys';

// Revisited virtual rows reuse their result; mutations explicitly invalidate it.
const publicationStaleTimeMs = 60_000;

export const entityPublicationOptions = (entityId: string) =>
  queryOptions({
    queryKey: entityQueryKeys.publication(entityId),
    queryFn: ({ signal }) => getEntityPublications(entityId, signal),
    staleTime: publicationStaleTimeMs,
  });
