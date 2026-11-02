import { useQuery } from '@tanstack/react-query';
import { useEffect } from 'react';
import { isClientErrorStatus } from '../../api/httpStatus';
import { ApiRequestError } from '../../api/request';
import { getExtensionRuntime, type ExtensionRuntimeScope } from './api';
import { extensionRuntimeRefetchInterval } from './constants';
import { extensionQueryKeys } from './queryKeys';

/**
 * Whether to show that extension content cannot load. A failed poll keeps the
 * last answer, and with it mounted frames and their state; a 4xx (revoked
 * access) contains them, as an empty answer would.
 */
export const isExtensionRuntimeUnavailable = ({
  data,
  error,
}: {
  data: unknown;
  error: unknown;
}) =>
  Boolean(error) &&
  (data === undefined ||
    (error instanceof ApiRequestError && isClientErrorStatus(error.status)));

export const useExtensionRuntime = (runtimeScope?: ExtensionRuntimeScope) => {
  const query = useQuery({
    queryKey: extensionQueryKeys.runtime(runtimeScope),
    queryFn: () => getExtensionRuntime(runtimeScope),
    // Runtime state can change outside this browser (safe mode, quarantine, or
    // grant revocation). Polling makes mounted frames unmount promptly; every
    // broker call remains server-gated between refreshes.
    refetchInterval: extensionRuntimeRefetchInterval,
    retry: false,
  });
  // Only the unscoped runtime lists every enabled release, so only it may
  // terminate native Workers whose release is gone or contained, and it
  // starts the rest before any of their contributions mounts. The native
  // runtime loads on demand, so it stays out of the app's initial JavaScript.
  useEffect(() => {
    const runtime = query.data;
    if (runtimeScope || !Array.isArray(runtime)) return;
    void import('./native/nativeRuntime').then(
      ({ containNativeHosts, startNativeHosts }) => {
        containNativeHosts(runtime);
        startNativeHosts(runtime);
      },
    );
  }, [runtimeScope, query.data]);
  return query;
};
