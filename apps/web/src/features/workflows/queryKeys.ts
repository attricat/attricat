export const workflowQueryKeys = {
  all: () => ['workflows'] as const,
  detail: (id: string) => [...workflowQueryKeys.all(), id] as const,
  revisions: (id: string) =>
    [...workflowQueryKeys.detail(id), 'revisions'] as const,
  revision: (id: string, version: number) =>
    [...workflowQueryKeys.revisions(id), version] as const,
  runs: () => [...workflowQueryKeys.all(), 'runs'] as const,
  workflowRuns: (workflowId: string) =>
    [...workflowQueryKeys.runs(), workflowId] as const,
  runSummaries: () => [...workflowQueryKeys.runs(), 'summaries'] as const,
} as const;
