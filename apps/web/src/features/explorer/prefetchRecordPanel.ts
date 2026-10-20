import type { QueryClient } from '@tanstack/react-query';
import type { BlueprintWithAttributes, RecordItem } from '../records/api';
import { recordQueryKeys } from '../records/queryKeys';
import { resolvedRecordPreviewOptions } from '../records/queryOptions';

/**
 * Starts loading what the record panel shows first, so that a record whose
 * link is pointed at or focused opens with its values. Records pinned to the
 * results' blueprint revision reuse it instead of loading it again.
 */
export const prefetchRecordPanel = (
  client: QueryClient,
  record: RecordItem,
  blueprint: BlueprintWithAttributes,
  contextId: string | undefined,
) => {
  const { id, version } = blueprint.blueprint;
  const revisionKey = recordQueryKeys.blueprintRevision(id, version);
  if (
    record.blueprint_version === version &&
    client.getQueryData(revisionKey) === undefined
  )
    client.setQueryData(revisionKey, blueprint);
  if (contextId)
    void client.prefetchQuery(
      resolvedRecordPreviewOptions(record.id, contextId),
    );
};
