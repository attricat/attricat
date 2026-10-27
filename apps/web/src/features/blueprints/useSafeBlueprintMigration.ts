import {
  blueprintMigrationBatchesOptions,
  blueprintRevisionOptions,
} from './queryOptions';
import { useQuery } from '@tanstack/react-query';
import { getSafeBlueprintMigrationImpact } from './api';
import { revisionStatuses } from '../../api/revisionStatus';
import { hasActiveMigrationForVersion } from './migrationBatches';
import { blueprintQueryKeys } from './queryKeys';
import { isSafeAutomaticMigration } from './safeMigration';
import type { Blueprint } from './schemas';

/**
 * Loads migration batches and decides whether records can be migrated
 * automatically from the previous published revision to the latest one.
 */
export const useSafeBlueprintMigration = (
  blueprintId: string,
  revisions: readonly Blueprint[],
) => {
  const latestPublished = revisions.find(
    (revision) => revision.status === revisionStatuses.published,
  );
  const targetVersion = latestPublished?.version;
  const sourceVersion =
    targetVersion === undefined
      ? undefined
      : revisions.find(
          (revision) =>
            revision.version < targetVersion &&
            revision.status === revisionStatuses.published,
        )?.version;
  const migrationBatches = useQuery(
    blueprintMigrationBatchesOptions(blueprintId),
  );
  const source = useQuery(blueprintRevisionOptions(blueprintId, sourceVersion));
  const target = useQuery(blueprintRevisionOptions(blueprintId, targetVersion));
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
