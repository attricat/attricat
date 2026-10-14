export const authQueryKeys = {
  all: () => ['auth'] as const,
  session: () => [...authQueryKeys.all(), 'session'] as const,
  accessAll: () => [...authQueryKeys.all(), 'access'] as const,
  access: (permission: string, kind: string, ids: readonly string[]) =>
    [...authQueryKeys.accessAll(), permission, kind, ids] as const,
  sampleLogins: () => [...authQueryKeys.all(), 'sample-logins'] as const,
} as const;
