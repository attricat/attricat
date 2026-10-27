/** What a role grant applies to: the whole workspace or one part of it. */
export const scopeTypes = {
  workspace: 'workspace',
  blueprintFamily: 'blueprint_family',
  record: 'record',
  contextSubtree: 'context_subtree',
} as const;
export const ownerRoleCode = 'owner';
/** A custom role code: lowercase, starting with a letter. */
export const roleCodePattern = /^[a-z][a-z0-9_-]*$/;
export const activeMemberState = 'active';
export const navigationSelectMinWidth = 250;
/** How the agent's bulk value edits are approved; see the API's settings. */
export const bulkEditPolicies = ['per_proposal', 'per_record'] as const;
/** The highest per-edit record limit a workspace may set. */
export const maxBulkEditMaxRecords = 5_000;
/** Width of the bulk edit limit field. */
export const bulkEditLimitFieldWidth = 220;
