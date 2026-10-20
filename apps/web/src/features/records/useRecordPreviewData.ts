import { useQuery } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import { getBlueprintRevision } from './api';
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
  const { t } = useTranslation();
  return useQuery({
    queryKey: recordQueryKeys.blueprintRevision(
      record?.blueprint_id,
      record?.blueprint_version,
    ),
    queryFn: () => {
      if (!record) throw new Error(t('records.recordPreviewUnavailable'));
      return getBlueprintRevision(
        record.blueprint_id,
        record.blueprint_version,
      );
    },
    enabled: Boolean(record?.blueprint_id && record.blueprint_version),
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
