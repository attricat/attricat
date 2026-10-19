import { keepPreviousData, useQueries } from '@tanstack/react-query';
import { ApiRequestError, searchRecords, type Blueprint } from '../records/api';
import { recordQueryKeys } from '../records/queryKeys';
import {
  defaultQueryRetries,
  invalidInputErrorCode,
  overviewRecordsPerBlueprint,
} from './constants';

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
        searchRecords({
          blueprint: blueprint.code,
          contextCode,
          includeTotal: true,
          pageSize: overviewRecordsPerBlueprint,
          query,
          signal,
          version: blueprint.version,
        }),
      placeholderData: keepPreviousData,
      // A rejection is the blueprint's answer, not a transient failure.
      retry: (failureCount: number, error: Error) =>
        !isRejectedQuery(error) && failureCount < defaultQueryRetries,
    })),
  });

/** Whether a blueprint refused the query rather than failing unexpectedly. */
export const isRejectedQuery = (error: unknown) =>
  error instanceof ApiRequestError && error.code === invalidInputErrorCode;
