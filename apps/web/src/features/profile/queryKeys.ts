export const profileQueryKeys = {
  all: () => ['profile'] as const,
  tokens: () => [...profileQueryKeys.all(), 'tokens'] as const,
  tokenPermissions: () =>
    [...profileQueryKeys.all(), 'token-permissions'] as const,
} as const;
