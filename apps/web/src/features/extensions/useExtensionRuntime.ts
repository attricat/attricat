import { useQuery } from '@tanstack/react-query';
import { getExtensionRuntime, type ExtensionRuntimeScope } from './api';
import { extensionRuntimeRefetchInterval } from './constants';
import { extensionQueryKeys } from './queryKeys';

export const useExtensionRuntime = (runtimeScope?: ExtensionRuntimeScope) =>
  useQuery({
    queryKey: extensionQueryKeys.runtime(runtimeScope),
    queryFn: () => getExtensionRuntime(runtimeScope),
    // Runtime state can change outside this browser (safe mode, quarantine, or
    // grant revocation). Polling makes mounted frames unmount promptly; every
    // broker call remains server-gated between refreshes.
    refetchInterval: extensionRuntimeRefetchInterval,
    retry: false,
  });
