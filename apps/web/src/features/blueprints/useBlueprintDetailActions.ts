import { useMutation, useQueryClient } from '@tanstack/react-query';
import { recordQueryKeys } from '../records/queryKeys';
import {
  publishBlueprintRecords,
  publishBlueprintRecordsAllChannels,
  publishBlueprintRevision,
  startSafeBlueprintMigrationBatch,
} from './api';
import { allPublicationChannels, type RemovalDisposition } from './constants';
import { blueprintQueryKeys } from './queryKeys';

/** Mutations started from the blueprint detail page and its dialogs. */
export const useBlueprintDetailActions = (
  blueprintId: string,
  {
    onRecordsPublished,
    onMigrationStarted,
    onPublished,
  }: {
    onRecordsPublished: () => void;
    onMigrationStarted: () => void;
    onPublished: () => void;
  },
) => {
  const queryClient = useQueryClient();
  const publish = useMutation({
    mutationFn: (version: number) =>
      publishBlueprintRevision(blueprintId, version),
    onSuccess: async (_, version) => {
      onPublished();
      await Promise.all([
        queryClient.invalidateQueries({
          queryKey: blueprintQueryKeys.catalogue(),
        }),
        queryClient.invalidateQueries({
          queryKey: blueprintQueryKeys.revisions(blueprintId),
        }),
        queryClient.invalidateQueries({
          queryKey: blueprintQueryKeys.revision(blueprintId, version),
        }),
      ]);
    },
  });
  const safeMigration = useMutation({
    mutationFn: ({
      version,
      removalDisposition,
    }: {
      version: number;
      removalDisposition?: RemovalDisposition;
    }) =>
      startSafeBlueprintMigrationBatch(
        blueprintId,
        version,
        removalDisposition,
      ),
    onSuccess: async () => {
      onMigrationStarted();
      await queryClient.invalidateQueries({
        queryKey: blueprintQueryKeys.migrationBatches(blueprintId),
      });
    },
  });
  const publishRecords = useMutation({
    mutationFn: ({
      version,
      contextId,
    }: {
      version: number;
      contextId: string;
    }) =>
      contextId === allPublicationChannels
        ? publishBlueprintRecordsAllChannels(blueprintId, version)
        : publishBlueprintRecords(blueprintId, version, contextId),
    onSuccess: async () => {
      onRecordsPublished();
      await queryClient.invalidateQueries({
        queryKey: recordQueryKeys.publications(),
      });
    },
  });

  return { publish, publishRecords, safeMigration };
};
