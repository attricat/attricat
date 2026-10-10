import { useQuery } from '@tanstack/react-query';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { listContexts } from '../contexts/api';
import { defaultContextCode } from '../contexts/constants';
import { contextQueryKeys } from '../contexts/queryKeys';
import { getResolvedRecordPreview } from './api';
import { recordQueryKeys } from './queryKeys';

/**
 * Loads attribute contexts and tracks the selected one. Until a context is
 * chosen, it follows `initialContextId` and then the Default context.
 */
export const useRecordContextSelection = (initialContextId?: string) => {
  const [selectedContext, setSelectedContext] = useState('');
  const contexts = useQuery({
    queryKey: contextQueryKeys.all(),
    queryFn: ({ signal }) => listContexts(signal),
  });
  const defaultContextId =
    contexts.data?.find((context) => context.code === defaultContextCode)?.id ??
    null;
  return {
    contextId: selectedContext || initialContextId || defaultContextId,
    contexts,
    defaultContextId,
    setSelectedContext,
  };
};

/** Resolves a record's values as seen from the given context. */
export const useResolvedRecordPreview = (
  recordId: string,
  contextId: string | null,
) => {
  const { t } = useTranslation();
  return useQuery({
    queryKey: recordQueryKeys.resolvedPreview(recordId, contextId ?? undefined),
    queryFn: () => {
      if (!contextId) throw new Error(t('records.previewContextUnavailable'));
      return getResolvedRecordPreview(recordId, contextId);
    },
    enabled: contextId !== null,
  });
};
