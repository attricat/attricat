import { useQuery } from '@tanstack/react-query';
import { useCan } from '../auth/useCan';
import { capabilities } from '../auth/capabilities';
import { extensionUpdates } from './managementApi';
import { extensionManagementQueryKeys } from './managementQueryKeys';

const extensionUpdatesStaleTimeMs = 5 * 60 * 1000;

/** Newer reviewed versions of installed extensions; none until they load. */
export const useExtensionUpdates = () => {
  const canRead = useCan(capabilities.extensionsRead);
  const updates = useQuery({
    queryKey: extensionManagementQueryKeys.updates(),
    queryFn: extensionUpdates,
    enabled: canRead,
    staleTime: extensionUpdatesStaleTimeMs,
  });
  return updates.data ?? [];
};
