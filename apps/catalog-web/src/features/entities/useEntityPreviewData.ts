import { useQuery } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import { getBlueprintRevision } from './api';
import { entityQueryKeys } from './queryKeys';
import {
  entityFormOptions,
  entityStatusTransitionsOptions,
} from './queryOptions';
import { useResolvedEntityPreview } from './useEntityContexts';

/**
 * Loads what an entity preview shows for the selected context: resolved
 * values, the blueprint revision they belong to, and the editable form.
 */
export const useEntityPreviewData = (
  entityId: string,
  contextId: string | null,
) => {
  const { t } = useTranslation();
  const resolved = useResolvedEntityPreview(entityId, contextId);
  const entityForm = useQuery({
    ...entityFormOptions(entityId),
    refetchOnMount: 'always',
  });
  // Explains which transitions this user may take; the server still decides.
  const statusTransitions = useQuery({
    ...entityStatusTransitionsOptions(entityId, contextId),
    enabled: contextId !== null && entityForm.data?.can_write === true,
  });
  const resolvedEntity = resolved.data?.entity;
  // The form carries the same revision, but it is refetched on every opening;
  // this cached revision is shared by every entity of the blueprint, so the
  // values render without waiting for the form.
  const blueprint = useQuery({
    queryKey: entityQueryKeys.blueprintRevision(
      resolvedEntity?.blueprint_id,
      resolvedEntity?.blueprint_version,
    ),
    queryFn: () => {
      if (!resolvedEntity)
        throw new Error(t('entities.entityPreviewUnavailable'));
      return getBlueprintRevision(
        resolvedEntity.blueprint_id,
        resolvedEntity.blueprint_version,
      );
    },
    enabled: Boolean(
      resolvedEntity?.blueprint_id && resolvedEntity.blueprint_version,
    ),
  });
  return { blueprint, entityForm, resolved, statusTransitions };
};
