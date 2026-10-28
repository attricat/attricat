import type { Capability } from './useCan';

/** Workspace permission codes, as the session reports them. */
export const capabilities = {
  auditRead: 'audit_read',
  dataHealthRead: 'data_health_read',
  membersManage: 'members_manage',
  rolesManage: 'roles_manage',
  rolesGrant: 'roles_grant',
  tokensManage: 'tokens_manage',
  workspaceNavigationManage: 'workspace_navigation_manage',
  tagColorsManage: 'tag_colors_manage',
  agentsManage: 'agents_manage',
  extensionsRead: 'extensions_read',
  extensionsManage: 'extensions_manage',
  workflowsRead: 'workflows_read',
  workflowsManage: 'workflows_manage',
  rulesRead: 'rules_read',
  rulesManage: 'rules_manage',
  recordsPublish: 'records_publish',
  recordsUnlock: 'records_unlock',
  recordsDelete: 'records_delete',
  blueprintsWrite: 'blueprints_write',
  recordsWrite: 'records_write',
  recordsRead: 'records_read',
  blueprintsPublish: 'blueprints_publish',
  contextsWrite: 'contexts_write',
  agentsRun: 'agents_run',
} as const satisfies Record<string, Capability>;

/** Targets whose scoped grants `checkAccess` evaluates. */
export const accessTargetKinds = {
  record: 'record',
  blueprint: 'blueprint',
  context: 'context',
} as const;
export type AccessTargetKind =
  (typeof accessTargetKinds)[keyof typeof accessTargetKinds];
