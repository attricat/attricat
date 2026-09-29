import { useQuery } from '@tanstack/react-query';
import {
  getBlueprintRevision,
  getSafeBlueprintMigrationImpact,
  listBlueprintMigrationBatches,
} from './api';
import { blueprintStatuses } from './constants';
import {
  ACTIVE_MIGRATION_POLL_INTERVAL_MS,
  hasActiveMigrationForVersion,
  isMigrationBatchActive,
} from './migrationBatches';
import { blueprintQueryKeys } from './queryKeys';
import { isSafeAutomaticMigration } from './safeMigration';
import type { Blueprint } from './schemas';

/**
 * Loads migration batches and decides whether entities can be migrated
 * automatically from the previous published revision to the latest one.
 */
export const useSafeBlueprintMigration = (
  blueprintId: string,
  revisions: readonly Blueprint[],
) => {
  const latestPublished = revisions.find(
    (revision) => revision.status === blueprintStatuses.published,
  );
  const targetVersion = latestPublished?.version;
  const sourceVersion =
    targetVersion === undefined
      ? undefined
      : revisions.find(
          (revision) =>
            revision.version < targetVersion &&
            revision.status === blueprintStatuses.published,
        )?.version;
  const migrationBatches = useQuery({
    queryKey: blueprintQueryKeys.migrationBatches(blueprintId),
    queryFn: () => listBlueprintMigrationBatches(blueprintId),
    refetchInterval: (query) =>
      query.state.data?.some(isMigrationBatchActive)
        ? ACTIVE_MIGRATION_POLL_INTERVAL_MS
        : false,
  });
  const source = useQuery({
    queryKey: blueprintQueryKeys.revision(blueprintId, sourceVersion ?? 0),
    queryFn: () => getBlueprintRevision(blueprintId, sourceVersion!),
    enabled: sourceVersion !== undefined,
  });
  const target = useQuery({
    queryKey: blueprintQueryKeys.revision(blueprintId, targetVersion ?? 0),
    queryFn: () => getBlueprintRevision(blueprintId, targetVersion!),
    enabled: targetVersion !== undefined,
  });
  const compatible =
    targetVersion !== undefined &&
    source.data !== undefined &&
    target.data !== undefined &&
    isSafeAutomaticMigration(source.data, target.data);
  const impact = useQuery({
    queryKey: blueprintQueryKeys.safeMigrationImpact(
      blueprintId,
      targetVersion ?? 0,
    ),
    queryFn: () => getSafeBlueprintMigrationImpact(blueprintId, targetVersion!),
    enabled: compatible,
  });

  return {
    canStart: compatible && impact.data !== undefined,
    currentVersionMigrationActive: hasActiveMigrationForVersion(
      migrationBatches.data,
      targetVersion,
    ),
    impact: impact.data,
    migrationBatches,
    sourceVersion,
    targetVersion,
    wasEvaluated:
      targetVersion !== undefined && source.isSuccess && target.isSuccess,
  };
};
