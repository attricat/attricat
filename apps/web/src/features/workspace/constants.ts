/** What a role grant applies to: the whole workspace or one part of it. */
export const scopeTypes = {
  workspace: 'workspace',
  blueprintFamily: 'blueprint_family',
  record: 'record',
  contextSubtree: 'context_subtree',
} as const;
export const ownerRoleCode = 'owner';
/** How long a new invitation stays open unless the inviter changes it. */
export const defaultInvitationLifetimeMilliseconds = 7 * 24 * 60 * 60 * 1000;
/** A custom role code: lowercase, starting with a letter. */
export const roleCodePattern = /^[a-z][a-z0-9_-]*$/;
export const activeMemberState = 'active';
export const navigationSelectMinWidth = 250;
/** Fits the longest color name with its swatch. */
export const tagColorSelectMinWidth = 180;
/** How the agent's bulk value edits are approved; see the API's settings. */
export const bulkEditPolicies = ['per_proposal', 'per_record'] as const;
/** The highest per-edit record limit a workspace may set. */
export const maxBulkEditMaxRecords = 5_000;
/** Width of the bulk edit limit field. */
export const bulkEditLimitFieldWidth = 220;

/** The longest team name the API accepts, in characters. */
export const teamNameMaxLength = 200;
/** The longest team code the API accepts; codes are ASCII, so bytes. */
export const teamCodeMaxLength = 128;

/** Who may sign in with a password once a workspace has a provider. */
export const passwordSignInPolicies = {
  all: 'all',
  owners: 'owners',
} as const;
