export const authQueryKeys = {
  all: () => ['auth'] as const,
  session: () => [...authQueryKeys.all(), 'session'] as const,
  sampleLogins: () => [...authQueryKeys.all(), 'sample-logins'] as const,
} as const;
