export const inspectorQueryKeys = {
  all: () => ['inspector'] as const,
  apiHealth: () => [...inspectorQueryKeys.all(), 'api-health'] as const,
};
