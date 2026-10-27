import type { QueryClient } from '@tanstack/react-query';
import { recordQueryKeys } from '../records/queryKeys';
import { blueprintQueryKeys } from './queryKeys';

/**
 * Refreshes the lists that show a blueprint after a revision is added or
 * published: the catalogue, its revisions, and the blueprint lists the
 * Explorer, navigation and relationship pickers keep until invalidated.
 */
export const invalidateBlueprintLists = (
  queryClient: QueryClient,
  blueprintId: string,
) =>
  Promise.all([
    queryClient.invalidateQueries({ queryKey: blueprintQueryKeys.catalogue() }),
    queryClient.invalidateQueries({
      queryKey: blueprintQueryKeys.revisions(blueprintId),
    }),
    queryClient.invalidateQueries({
      queryKey: recordQueryKeys.blueprintsByCode(),
    }),
    queryClient.invalidateQueries({ queryKey: recordQueryKeys.blueprints() }),
  ]);
