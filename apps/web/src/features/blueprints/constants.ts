import type { BlueprintMigrationBatchStatus } from './schemas';

export const migrationBatchStatuses = {
  draft: 'draft',
  queued: 'queued',
  running: 'running',
  completed: 'completed',
  failed: 'failed',
  superseded: 'superseded',
} as const satisfies Record<string, BlueprintMigrationBatchStatus['status']>;

/** What a blueprint describes: records, or attributes other blueprints include. */
export const blueprintKinds = {
  record: 'record',
  mixin: 'mixin',
} as const;

/** Approves archival of stored values for attributes removed by a migration. */
export const archiveRemovalDisposition = 'archive';
export type RemovalDisposition = typeof archiveRemovalDisposition;

/** Publication channel option that publishes to every enabled channel. */
export const allPublicationChannels = 'all';

/** Version of the context payload handed to blueprint extension outlets. */

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
  recordSchema: 3,
  includes: 4,
  publicationPolicy: 5,
} as const;
export type BlueprintVersionMetadataTab =
  (typeof blueprintVersionMetadataTabs)[keyof typeof blueprintVersionMetadataTabs];

/** Sandbox tab that edits sample values through the detail layout. */
export const sandboxEditTab = 'sandbox';

export const blueprintFilterWidth = 420;
export const tomlDiffEditorHeight = 560;
export const migrationProgressMinWidth = 150;
export const completePercentage = 100;
