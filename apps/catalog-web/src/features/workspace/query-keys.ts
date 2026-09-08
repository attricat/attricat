export const workspaceQueryKeys = {
  all: () => ['workspace'] as const,
  members: () => [...workspaceQueryKeys.all(), 'members'] as const,
  invitations: () => [...workspaceQueryKeys.all(), 'invitations'] as const,
  roles: () => [...workspaceQueryKeys.all(), 'roles'] as const,
  permissions: () => [...workspaceQueryKeys.all(), 'permissions'] as const,
  exploreNavigation: () =>
    [...workspaceQueryKeys.all(), 'explore-navigation'] as const,
  sidebarExploreNavigation: () =>
    [...workspaceQueryKeys.all(), 'sidebar-explore-navigation'] as const,
  assignableRoles: () =>
    [...workspaceQueryKeys.all(), 'assignable-roles'] as const,
  grantTargets: (scope: string) =>
    [...workspaceQueryKeys.all(), 'grant-targets', scope] as const,
} as const;
