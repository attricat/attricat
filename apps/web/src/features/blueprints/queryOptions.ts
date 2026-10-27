import { queryOptions, skipToken } from '@tanstack/react-query';
import {
  getBlueprintRevision,
  listBlueprintMigrationBatches,
  listBlueprintRevisions,
  listBlueprints,
} from './api';
import {
  ACTIVE_MIGRATION_POLL_INTERVAL_MS,
  isMigrationBatchActive,
} from './migrationBatches';
import { blueprintQueryKeys } from './queryKeys';

/** The workspace's blueprints, latest revision of each. */
export const blueprintCatalogueOptions = () =>
  queryOptions({
    queryKey: blueprintQueryKeys.catalogue(),
    queryFn: listBlueprints,
  });

/** One revision of a blueprint with its attributes, once both are known. */
export const blueprintRevisionOptions = (
  id: string | undefined,
  version: number | undefined,
) =>
  queryOptions({
    queryKey: blueprintQueryKeys.revision(id, version),
    queryFn:
      id && version
        ? ({ signal }) => getBlueprintRevision(id, version, signal)
        : skipToken,
  });

/** Every revision of a blueprint, newest first, once it is known. */
export const blueprintRevisionListOptions = (id: string | undefined) =>
  queryOptions({
    queryKey: blueprintQueryKeys.revisions(id ?? ''),
    queryFn: id
      ? ({ signal }) => listBlueprintRevisions(id, signal)
      : skipToken,
  });

/** A blueprint's migration batches, polled while any of them is active. */
export const blueprintMigrationBatchesOptions = (id: string) =>
  queryOptions({
    queryKey: blueprintQueryKeys.migrationBatches(id),
    queryFn: () => listBlueprintMigrationBatches(id),
    refetchInterval: (query) =>
      query.state.data?.some(isMigrationBatchActive)
        ? ACTIVE_MIGRATION_POLL_INTERVAL_MS
        : false,
  });
