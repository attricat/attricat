export const workspaceQueryKeys = {
  members: () => ['workspace', 'members'] as const,
  invitations: () => ['workspace', 'invitations'] as const,
  roles: () => ['workspace', 'roles'] as const,
  permissions: () => ['workspace', 'permissions'] as const,
  assignableRoles: () => ['workspace', 'assignable-roles'] as const,
  tokenPermissions: () => ['workspace', 'token-permissions'] as const,
  grantTargets: (scope: string) =>
    ['workspace', 'grant-targets', scope] as const,
  tokens: () => ['workspace', 'tokens'] as const,
} as const;
