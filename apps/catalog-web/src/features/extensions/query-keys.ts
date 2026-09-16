export const extensionQueryKeys = {
  all: ['extensions'] as const,
  runtime: (scope?: { blueprintId: string; blueprintVersion: number }) =>
    ['extensions', 'runtime', scope ?? null] as const,
};
