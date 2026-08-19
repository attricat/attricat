export const dataHealthQueryKeys = {
  summary: (staleAfterDays: number) =>
    ['data-health', 'summary', staleAfterDays] as const,
  blueprints: (staleAfterDays: number) =>
    ['data-health', 'blueprints', staleAfterDays] as const,
  freshness: () => ['data-health', 'freshness'] as const,
  completeness: () => ['data-health', 'completeness'] as const,
  contexts: () => ['data-health', 'contexts'] as const,
  relationships: () => ['data-health', 'relationships'] as const,
  storage: () => ['data-health', 'storage'] as const,
} as const;
