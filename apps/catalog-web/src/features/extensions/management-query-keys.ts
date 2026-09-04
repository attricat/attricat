export const extensionManagementQueryKeys = {
  all: ['extension-management'] as const,
  marketplace: () => ['extension-management', 'marketplace'] as const,
  registry: (owner: string, repository: string) =>
    ['extension-management', 'registry', owner, repository] as const,
  installed: () => ['extension-management', 'installed'] as const,
  detail: (id: string) => ['extension-management', 'detail', id] as const,
};
