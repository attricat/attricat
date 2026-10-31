import { useQuery } from '@tanstack/react-query';
import { useEffect } from 'react';
import { getExtensionRuntime } from '../api';
import { extensionQueryKeys } from '../queryKeys';
import { startNativeHosts } from './nativeRuntime';

/**
 * Part of the app shell: reads the runtime once when the app starts and
 * starts every enabled native extension's Worker, so the shared sandbox
 * frame and each Worker load once per app start rather than on the first
 * page that shows native UI. Extension surfaces keep the 15-second refresh
 * that contains Workers; this reader adds no polling of its own.
 */
export const NativeWarmup = () => {
  const runtime = useQuery({
    queryKey: extensionQueryKeys.runtime(),
    queryFn: () => getExtensionRuntime(),
    retry: false,
  });
  useEffect(() => {
    if (Array.isArray(runtime.data)) startNativeHosts(runtime.data);
  }, [runtime.data]);
  return null;
};
