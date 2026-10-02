import type { Blueprint, BlueprintMigrationBatchStatus } from './schemas';

export const blueprintStatuses = {
  draft: 'draft',
  published: 'published',
} as const satisfies Record<string, Blueprint['status']>;

export const blueprintStatusChipColor = (status: Blueprint['status']) =>
  status === blueprintStatuses.published ? 'success' : 'warning';

export const migrationBatchStatuses = {
  draft: 'draft',
  queued: 'queued',
  running: 'running',
  completed: 'completed',
  failed: 'failed',
  superseded: 'superseded',
} as const satisfies Record<string, BlueprintMigrationBatchStatus['status']>;

/** Approves archival of stored values for attributes removed by a migration. */
export const archiveRemovalDisposition = 'archive';
export type RemovalDisposition = typeof archiveRemovalDisposition;

/** Publication channel option that publishes to every enabled channel. */
export const allPublicationChannels = 'all';

/** Version of the context payload handed to blueprint extension outlets. */
export const blueprintExtensionContextVersion = 1;

export const blueprintDetailTabs = {
  metadata: 0,
  revisionHistory: 1,
  compareDefinitions: 2,
  migrations: 3,
} as const;
export type BlueprintDetailTab =
  (typeof blueprintDetailTabs)[keyof typeof blueprintDetailTabs];

export const blueprintVersionMetadataTabs = {
  attributes: 0,
  views: 1,
  viewDefinition: 2,
  entitySchema: 3,
  includes: 4,
  publicationPolicy: 5,
} as const;
export type BlueprintVersionMetadataTab =
  (typeof blueprintVersionMetadataTabs)[keyof typeof blueprintVersionMetadataTabs];

/** Name of the blueprint view used for entity forms. */
export const editViewName = 'edit';

/** Displayed for values that are absent, such as an empty removal policy. */
export const emptyValuePlaceholder = '—';

export const blueprintFilterWidth = 420;
export const tomlDiffEditorHeight = 560;
export const blueprintEditorHeight = 'calc(100vh - 260px)';
export const migrationProgressMinWidth = 150;
export const completePercentage = 100;
