import { queryOptions } from '@tanstack/react-query';
import { extensionSecrets } from './managementApi';
import { extensionManagementQueryKeys } from './managementQueryKeys';

/** The secrets one installed extension declares and has set, without values. */
export const extensionSecretsOptions = (extensionId: string) =>
  queryOptions({
    queryKey: extensionManagementQueryKeys.secrets(extensionId),
    queryFn: () => extensionSecrets(extensionId),
  });
