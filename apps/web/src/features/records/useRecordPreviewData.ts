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
 * Loads what a record preview shows for the selected context: resolved
 * values, the blueprint revision they belong to, and the editable form.
 */
export const useRecordPreviewData = (
  recordId: string,
  contextId: string | null,
) => {
  const { t } = useTranslation();
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
  const resolvedRecord = resolved.data?.record;
  // The form carries the same revision, but it is refetched on every opening;
  // this cached revision is shared by every record of the blueprint, so the
  // values render without waiting for the form.
  const blueprint = useQuery({
    queryKey: recordQueryKeys.blueprintRevision(
      resolvedRecord?.blueprint_id,
      resolvedRecord?.blueprint_version,
    ),
    queryFn: () => {
      if (!resolvedRecord)
        throw new Error(t('records.recordPreviewUnavailable'));
      return getBlueprintRevision(
        resolvedRecord.blueprint_id,
        resolvedRecord.blueprint_version,
      );
    },
    enabled: Boolean(
      resolvedRecord?.blueprint_id && resolvedRecord.blueprint_version,
    ),
  });
  return { blueprint, recordForm, resolved, statusTransitions };
};
