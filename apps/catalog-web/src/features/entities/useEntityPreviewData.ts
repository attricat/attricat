import { useQuery } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import { getBlueprintRevision, getEntityForm } from './api';
import { entityQueryKeys } from './queryKeys';
import { entityStatusTransitionsOptions } from './queryOptions';
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
    queryKey: entityQueryKeys.form(entityId),
    queryFn: ({ signal }) => getEntityForm(entityId, signal),
    refetchOnMount: 'always',
  });
  // Explains which transitions this user may take; the server still decides.
  const statusTransitions = useQuery({
    ...entityStatusTransitionsOptions(entityId, contextId),
    enabled: contextId !== null && entityForm.data?.can_write === true,
  });
  const resolvedEntity = resolved.data?.entity;
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
