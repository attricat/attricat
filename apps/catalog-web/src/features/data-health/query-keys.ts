export const dataHealthQueryKeys = {
  all: () => ['data-health'] as const,
  summary: (staleAfterDays: number) =>
    [...dataHealthQueryKeys.all(), 'summary', staleAfterDays] as const,
  blueprints: (staleAfterDays: number) =>
    [...dataHealthQueryKeys.all(), 'blueprints', staleAfterDays] as const,
  freshness: () => [...dataHealthQueryKeys.all(), 'freshness'] as const,
  completeness: () => [...dataHealthQueryKeys.all(), 'completeness'] as const,
  contexts: () => [...dataHealthQueryKeys.all(), 'contexts'] as const,
  relationships: () => [...dataHealthQueryKeys.all(), 'relationships'] as const,
  storage: () => [...dataHealthQueryKeys.all(), 'storage'] as const,
} as const;
