export const ruleQueryKeys = {
  all: ['rules'] as const,
  definitions: () => [...ruleQueryKeys.all, 'definitions'] as const,
  findings: (recordId?: string) =>
    recordId
      ? ([...ruleQueryKeys.all, 'findings', recordId] as const)
      : ([...ruleQueryKeys.all, 'findings'] as const),
  runs: () => [...ruleQueryKeys.all, 'runs'] as const,
};
