export const userQueryKeys = {
  all: () => ['users'] as const,
  profile: (userId: string) =>
    [...userQueryKeys.all(), 'profile', userId] as const,
} as const;
