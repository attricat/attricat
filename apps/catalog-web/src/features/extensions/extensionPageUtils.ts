import type { QueryClient } from '@tanstack/react-query';
import { extensionQueryKeys } from './queryKeys';
import { extensionManagementQueryKeys } from './managementQueryKeys';

export const repositoryParts = (repository: string) =>
  repository.replace(/^github:/, '').split('/', 2);

export const invalidateExtensions = (
  client: QueryClient,
  extensionId?: string,
) => {
  void client.invalidateQueries({ queryKey: extensionManagementQueryKeys.all });
  void client.invalidateQueries({ queryKey: extensionQueryKeys.runtimeRoot() });
  if (extensionId)
    void client.invalidateQueries({
      queryKey: extensionManagementQueryKeys.detail(extensionId),
    });
};
