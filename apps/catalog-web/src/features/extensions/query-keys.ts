export const extensionQueryKeys = {
  all: ['extensions'] as const,
  runtime: () => ['extensions', 'runtime'] as const,
};
