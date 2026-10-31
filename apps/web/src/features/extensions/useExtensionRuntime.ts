import { useQuery } from '@tanstack/react-query';
import { useEffect } from 'react';
import { getExtensionRuntime, type ExtensionRuntimeScope } from './api';
import { extensionRuntimeRefetchInterval } from './constants';
import { extensionQueryKeys } from './queryKeys';

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
