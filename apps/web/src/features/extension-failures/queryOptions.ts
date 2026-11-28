import { infiniteQueryOptions } from '@tanstack/react-query';
import { nextKeysetCursor, type KeysetCursor } from '../../api/keysetCursor';
import {
  deliveryFailureCursor,
  listDeliveryFailures,
  listRunFailures,
  runFailureCursor,
  type DeliveryCursor,
} from './api';
import { extensionFailureQueryKeys } from './queryKeys';

/** Pages of one installed extension's failed and retrying runs. */
export const runFailuresOptions = (extensionId: string) =>
  infiniteQueryOptions({
    queryKey: extensionFailureQueryKeys.runs(extensionId),
    queryFn: ({ pageParam }) => listRunFailures(extensionId, pageParam),
    initialPageParam: undefined as KeysetCursor | undefined,
    getNextPageParam: (page) => nextKeysetCursor(page, runFailureCursor),
  });

/** Pages of the event deliveries its handlers rejected or gave up on. */
export const deliveryFailuresOptions = (extensionId: string) =>
  infiniteQueryOptions({
    queryKey: extensionFailureQueryKeys.deliveries(extensionId),
    queryFn: ({ pageParam }) => listDeliveryFailures(extensionId, pageParam),
    initialPageParam: undefined as DeliveryCursor | undefined,
    getNextPageParam: (page) => nextKeysetCursor(page, deliveryFailureCursor),
  });
