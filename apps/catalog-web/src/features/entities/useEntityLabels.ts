import { useQueries } from '@tanstack/react-query';
import { getEntityLabels } from './api';
import { ENTITY_LABEL_BATCH_SIZE } from './constants';
import { entityQueryKeys } from './queryKeys';

/**
 * Default-context display labels of entities known only by ID, looked up in
 * batches. Entities without a label, or that cannot be read, are left out.
 */
export const useEntityLabels = (
  entityIds: readonly string[],
): ReadonlyMap<string, string> => {
  const ids = [...new Set(entityIds)].sort();
  const batches = Array.from(
    { length: Math.ceil(ids.length / ENTITY_LABEL_BATCH_SIZE) },
    (_, index) =>
      ids.slice(
        index * ENTITY_LABEL_BATCH_SIZE,
        (index + 1) * ENTITY_LABEL_BATCH_SIZE,
      ),
  );
  return useQueries({
    queries: batches.map((batch) => ({
      queryKey: entityQueryKeys.labels(batch),
      queryFn: ({ signal }: { signal: AbortSignal }) =>
        getEntityLabels(batch, signal),
    })),
    combine: (results) =>
      new Map(
        results.flatMap((result) =>
          (result.data?.items ?? []).flatMap((item) =>
            item.display.default ? [[item.id, item.display.default]] : [],
          ),
        ),
      ),
  });
};
