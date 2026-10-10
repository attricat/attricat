export const principalQueryKeys = {
  all: () => ['principals'] as const,
  directory: () => [...principalQueryKeys.all(), 'directory'] as const,
  teams: () => [...principalQueryKeys.all(), 'teams'] as const,
} as const;
