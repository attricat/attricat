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
  updates: () => [...extensionManagementQueryKeys.all, 'updates'] as const,
  layout: () => [...extensionManagementQueryKeys.all, 'layout'] as const,
  secrets: (extensionId: string) =>
    [...extensionManagementQueryKeys.all, 'secrets', extensionId] as const,
  detail: (id: string) =>
    [...extensionManagementQueryKeys.all, 'detail', id] as const,
  /** Under the detail, so whatever refreshes the installation refreshes it. */
  lifecycle: (id: string) =>
    [...extensionManagementQueryKeys.detail(id), 'lifecycle'] as const,
};
