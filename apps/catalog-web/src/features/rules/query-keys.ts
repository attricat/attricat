export const ruleQueryKeys = {
  all: ['rules'] as const,
  definitions: () => [...ruleQueryKeys.all, 'definitions'] as const,
  findings: () => [...ruleQueryKeys.all, 'findings'] as const,
  runs: () => [...ruleQueryKeys.all, 'runs'] as const,
};
