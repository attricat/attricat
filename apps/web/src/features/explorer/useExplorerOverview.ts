import { keepPreviousData, useQueries } from '@tanstack/react-query';
import { ApiRequestError, searchRecords, type Blueprint } from '../records/api';
import { recordQueryKeys } from '../records/queryKeys';
import {
  defaultQueryRetries,
  invalidInputErrorCode,
  overviewConcurrentSearches,
  overviewRecordsPerBlueprint,
  overviewStaleTimeMs,
} from './constants';

// One search per blueprint would otherwise all reach the server at once.
let runningSearches = 0;
const waitingSearches: Array<() => void> = [];

const releaseSearch = () => {
  runningSearches -= 1;
  waitingSearches.shift()?.();
};

/** Runs `search` once fewer than the allowed overview searches are running. */
const limitedSearch = async <T>(
  signal: AbortSignal,
  search: () => Promise<T>,
): Promise<T> => {
  signal.throwIfAborted();
  if (runningSearches >= overviewConcurrentSearches)
    await new Promise<void>((resolve, reject) => {
      const start = () => {
        signal.removeEventListener('abort', cancel);
        resolve();
      };
      const cancel = () => {
        waitingSearches.splice(waitingSearches.indexOf(start), 1);
        reject(signal.reason);
      };
      waitingSearches.push(start);
      signal.addEventListener('abort', cancel, { once: true });
    });
  runningSearches += 1;
  try {
    return await search();
  } finally {
    releaseSearch();
  }
};

/**
 * The first matching records of every blueprint. A query can name fields that
 * only some blueprints have; the others reject it as invalid input.
 */
export const useExplorerOverview = (
  blueprints: Blueprint[],
  { contextCode, query }: { contextCode: string; query: string },
) =>
  useQueries({
    queries: blueprints.map((blueprint) => ({
      queryKey: recordQueryKeys.blueprintOverview({
        blueprint: blueprint.code,
        contextCode,
        pageSize: overviewRecordsPerBlueprint,
        query,
        version: blueprint.version,
      }),
      queryFn: ({ signal }: { signal: AbortSignal }) =>
        limitedSearch(signal, () =>
          searchRecords({
            blueprint: blueprint.code,
            contextCode,
            includeTotal: true,
            pageSize: overviewRecordsPerBlueprint,
            query,
            signal,
            version: blueprint.version,
          }),
        ),
      placeholderData: keepPreviousData,
      staleTime: overviewStaleTimeMs,
      // A rejection is the blueprint's answer, not a transient failure.
      retry: (failureCount: number, error: Error) =>
        !isRejectedQuery(error) && failureCount < defaultQueryRetries,
    })),
  });

/** Whether a blueprint refused the query rather than failing unexpectedly. */
export const isRejectedQuery = (error: unknown) =>
  error instanceof ApiRequestError && error.code === invalidInputErrorCode;
