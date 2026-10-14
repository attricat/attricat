import type { ScopeType } from './api';

export const workspaceScope: ScopeType = 'workspace';
export const ownerRoleCode = 'owner';
export const activeMemberState = 'active';
export const navigationSelectMinWidth = 250;
/** How the agent's bulk value edits are approved; see the API's settings. */
export const bulkEditPolicies = ['per_proposal', 'per_record'] as const;
/** The highest per-edit record limit a workspace may set. */
export const maxBulkEditMaxRecords = 5_000;
/** Width of the bulk edit limit field. */
export const bulkEditLimitFieldWidth = 220;
