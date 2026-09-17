import type { BlueprintMigrationBatchStatus } from './schemas';

export const ACTIVE_MIGRATION_POLL_INTERVAL_MS = 2_000;

export const isMigrationBatchActive = (
  batch: BlueprintMigrationBatchStatus,
): boolean => batch.status === 'queued' || batch.status === 'running';

export const hasActiveMigrationForVersion = (
  batches: BlueprintMigrationBatchStatus[] | undefined,
  version: number | undefined,
): boolean =>
  version !== undefined &&
  batches?.some(
    (batch) =>
      batch.target_version === version && isMigrationBatchActive(batch),
  ) === true;
