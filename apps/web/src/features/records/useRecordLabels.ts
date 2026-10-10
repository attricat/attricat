import { useQueries } from '@tanstack/react-query';
import { getRecordLabels } from './api';
import { RECORD_LABEL_BATCH_SIZE } from './constants';
import { recordQueryKeys } from './queryKeys';

/**
 * Default-context display labels of records known only by ID, looked up in
 * batches. Records without a label, or that cannot be read, are left out.
 */
export const useRecordLabels = (
  recordIds: readonly string[],
): ReadonlyMap<string, string> => {
  const ids = [...new Set(recordIds)].sort();
  const batches = Array.from(
    { length: Math.ceil(ids.length / RECORD_LABEL_BATCH_SIZE) },
    (_, index) =>
      ids.slice(
        index * RECORD_LABEL_BATCH_SIZE,
        (index + 1) * RECORD_LABEL_BATCH_SIZE,
      ),
  );
  return useQueries({
    queries: batches.map((batch) => ({
      queryKey: recordQueryKeys.labels(batch),
      queryFn: ({ signal }: { signal: AbortSignal }) =>
        getRecordLabels(batch, signal),
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
