export const workflowStatus = {
  draft: 'draft',
  published: 'published',
} as const;

export const workflowRunStatus = {
  completed: 'completed',
  deadLetter: 'dead_letter',
} as const;

export const workflowDetailTab = {
  revisions: 0,
  source: 1,
  compare: 2,
  runDiagnostics: 3,
} as const;

export type WorkflowDetailTab =
  (typeof workflowDetailTab)[keyof typeof workflowDetailTab];

export const workflowCapabilities = {
  read: 'workflows_read',
  manage: 'workflows_manage',
} as const;

export const workflowRoutes = {
  list: '/manage/workflows',
  create: '/manage/workflows/new',
  detail: '/manage/workflows/$workflowId',
  newRevision: '/manage/workflows/$workflowId/revisions/$version/new',
} as const;

export const compareSelectMinWidth = 220;
export const runTableEmptyColSpan = 6;
export const editorHeight = 'calc(100vh - 380px)';
export const monospaceFontFamily = 'monospace';

export const starterWorkflowDefinition = `format_version = 1
code = "example-workflow"
name = "Example workflow"

[[triggers]]
event_type = "entity.created.v1"

[[actions]]
type = "system_tags_add"
tags = ["example"]
`;
