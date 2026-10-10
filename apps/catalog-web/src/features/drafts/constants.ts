// Prefix shared by every tab-scoped editor draft in session storage.
export const draftStorageKeyPrefix = 'catalog.draft';

// Bump when the stored envelope changes shape; older drafts are then ignored.
export const draftFormatVersion = 1;

// Largest serialized draft kept, in UTF-16 code units, so a single editor
// cannot exhaust the origin's session storage quota.
export const maxDraftLength = 512 * 1024;

// Delay before the latest edit is written to session storage.
export const draftWriteDelayMs = 500;

export const draftEditors = {
  blueprintCreate: 'blueprint-create',
  blueprintRevision: 'blueprint-revision',
  recordCreate: 'record-create',
  recordComment: 'record-comment',
  reusableAttributeCreate: 'reusable-attribute-create',
  reusableAttributeRevision: 'reusable-attribute-revision',
  workflowCreate: 'workflow-create',
  workflowRevision: 'workflow-revision',
} as const;

export type DraftEditor = (typeof draftEditors)[keyof typeof draftEditors];
