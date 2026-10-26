export const extensionManagementQueryKeys = {
  all: ['extension-management'] as const,
  marketplace: () =>
    [...extensionManagementQueryKeys.all, 'marketplace'] as const,
  registry: (owner: string, repository: string) =>
    [
      ...extensionManagementQueryKeys.all,
      'registry',
      owner,
      repository,
    ] as const,
  installed: () => [...extensionManagementQueryKeys.all, 'installed'] as const,
  layout: () => [...extensionManagementQueryKeys.all, 'layout'] as const,
  detail: (id: string) =>
    [...extensionManagementQueryKeys.all, 'detail', id] as const,
};
