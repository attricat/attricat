export const profileQueryKeys = {
  tokens: () => ['profile', 'tokens'] as const,
  tokenPermissions: () => ['profile', 'token-permissions'] as const,
} as const;
