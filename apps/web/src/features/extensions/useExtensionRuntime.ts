import { useQuery } from '@tanstack/react-query';
import { useEffect } from 'react';
import { getExtensionRuntime, type ExtensionRuntimeScope } from './api';
import { extensionRuntimeRefetchInterval } from './constants';
import { containNativeHosts, startNativeHosts } from './native/nativeRuntime';
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
  // starts the rest before any of their contributions mounts.
  useEffect(() => {
    if (runtimeScope || !Array.isArray(query.data)) return;
    containNativeHosts(query.data);
    startNativeHosts(query.data);
  }, [runtimeScope, query.data]);
  return query;
};
