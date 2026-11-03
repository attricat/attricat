import { queryOptions } from '@tanstack/react-query';
import { z } from 'zod';
import { notFoundErrorCode } from '../../api/errors';
import { httpStatus } from '../../api/httpStatus';
import { ApiRequestError } from '../../api/request';
import { getExtensionRun, listExtensionRuns } from './api';
import { extensionRunQueryKeys } from './queryKeys';
import {
  extensionRunRefetchInterval,
  isActiveExtensionRun,
} from './runPolling';

/** One run, polled until it finishes. */
export const extensionRunDetailOptions = (runId: string) =>
  queryOptions({
    queryKey: extensionRunQueryKeys.detail(runId),
    queryFn: () => {
      // A malformed ID in the address names no run, which must not be polled.
      if (!z.uuid().safeParse(runId).success)
        throw new ApiRequestError(
          httpStatus.notFound,
          'Not found',
          notFoundErrorCode,
        );
      return getExtensionRun(runId);
    },
    refetchInterval: ({ state: { data, error } }) =>
      extensionRunRefetchInterval(!data || isActiveExtensionRun(data), error),
  });

/** The signed-in user's recent runs, polled while any of them is active. */
export const extensionRunListOptions = () =>
  queryOptions({
    queryKey: extensionRunQueryKeys.list(),
    queryFn: () => listExtensionRuns(),
    refetchInterval: ({ state: { data, error } }) =>
      extensionRunRefetchInterval(
        Boolean(data?.some(isActiveExtensionRun)),
        error,
      ),
  });
