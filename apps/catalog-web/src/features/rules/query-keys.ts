export const ruleQueryKeys = {
  all: ['rules'] as const,
  definitions: () => [...ruleQueryKeys.all, 'definitions'] as const,
  findings: (entityId?: string) =>
    entityId
      ? ([...ruleQueryKeys.all, 'findings', entityId] as const)
      : ([...ruleQueryKeys.all, 'findings'] as const),
  runs: () => [...ruleQueryKeys.all, 'runs'] as const,
};
