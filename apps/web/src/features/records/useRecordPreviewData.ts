import { skipToken, useQuery, useQueryClient } from '@tanstack/react-query';
import { getBlueprintRevision, type BlueprintWithAttributes } from './api';
import { recordQueryKeys } from './queryKeys';
import {
  recordFormOptions,
  recordStatusTransitionsOptions,
} from './queryOptions';
import { useResolvedRecordPreview } from './useRecordContexts';

/**
 * Loads the blueprint revision a record's values belong to. The revision is
 * cached for every record of the blueprint.
 */
export const useRecordBlueprintRevision = (
  record: { blueprint_id: string; blueprint_version: number } | undefined,
) => {
  const client = useQueryClient();
  return useQuery({
    queryKey: recordQueryKeys.blueprintRevision(
      record?.blueprint_id,
      record?.blueprint_version,
    ),
    queryFn: record
      ? () =>
          getBlueprintRevision(record.blueprint_id, record.blueprint_version)
      : skipToken,
    // Published revisions never change.
    staleTime: Infinity,
    // Most records use their blueprint's latest revision, which pages such as
    // a relationship field's picker have often loaded by code already.
    initialData: () =>
      record &&
      client
        .getQueriesData<BlueprintWithAttributes>({
          queryKey: recordQueryKeys.blueprintsByCode(),
        })
        .map(([, data]) => data)
        .find(
          (data) =>
            data?.blueprint.id === record.blueprint_id &&
            data.blueprint.version === record.blueprint_version,
        ),
  });
};

/**
 * Loads what a record preview shows for the selected context: resolved
 * values, the blueprint revision they belong to, and the editable form.
 */
export const useRecordPreviewData = (
  recordId: string,
  contextId: string | null,
) => {
  const resolved = useResolvedRecordPreview(recordId, contextId);
  const recordForm = useQuery({
    ...recordFormOptions(recordId),
    refetchOnMount: 'always',
  });
  // Explains which transitions this user may take; the server still decides.
  const statusTransitions = useQuery({
    ...recordStatusTransitionsOptions(recordId, contextId),
    enabled: contextId !== null && recordForm.data?.can_write === true,
  });
  // The form carries the same revision, but it is refetched on every opening;
  // the cached revision lets the values render without waiting for the form.
  const blueprint = useRecordBlueprintRevision(resolved.data?.record);
  return { blueprint, recordForm, resolved, statusTransitions };
};
