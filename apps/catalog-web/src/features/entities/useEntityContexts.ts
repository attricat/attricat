import { useQuery } from '@tanstack/react-query';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { listContexts } from '../contexts/api';
import { defaultContextCode } from '../contexts/constants';
import { contextQueryKeys } from '../contexts/queryKeys';
import { getResolvedEntityPreview } from './api';
import { entityQueryKeys } from './queryKeys';

/** Loads attribute contexts and tracks the selected one, defaulting to Default. */
export const useEntityContextSelection = () => {
  const [selectedContext, setSelectedContext] = useState('');
  const contexts = useQuery({
    queryKey: contextQueryKeys.all(),
    queryFn: ({ signal }) => listContexts(signal),
  });
  const defaultContextId =
    contexts.data?.find((context) => context.code === defaultContextCode)?.id ??
    null;
  return {
    contextId: selectedContext || defaultContextId,
    contexts,
    defaultContextId,
    setSelectedContext,
  };
};

/** Resolves an entity's values as seen from the given context. */
export const useResolvedEntityPreview = (
  entityId: string,
  contextId: string | null,
) => {
  const { t } = useTranslation();
  return useQuery({
    queryKey: entityQueryKeys.resolvedPreview(entityId, contextId ?? undefined),
    queryFn: () => {
      if (!contextId) throw new Error(t('entities.previewContextUnavailable'));
      return getResolvedEntityPreview(entityId, contextId);
    },
    enabled: contextId !== null,
  });
};
