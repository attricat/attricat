import { useQuery } from '@tanstack/react-query';
import { useState } from 'react';
import { findDefaultContextId } from '../contexts/defaultContext';
import { resolvedRecordPreviewOptions } from './queryOptions';
import { contextListOptions } from '../contexts/queryOptions';

/**
 * Loads attribute contexts and tracks the selected one. Until a context is
 * chosen, it follows `initialContextId` and then the Default context.
 */
export const useRecordContextSelection = (initialContextId?: string) => {
  const [selectedContext, setSelectedContext] = useState('');
  const contexts = useQuery(contextListOptions());
  const defaultContextId = findDefaultContextId(contexts.data);
  // A linked context that no longer exists falls back to the default one.
  // Like the default context, it is known once the contexts load.
  const initial = contexts.data?.some(
    (context) => context.id === initialContextId,
  )
    ? initialContextId
    : undefined;
  return {
    contextId: selectedContext || initial || defaultContextId,
    contexts,
    defaultContextId,
    setSelectedContext,
  };
};

/** Resolves a record's values as seen from the given context. */
export const useResolvedRecordPreview = (
  recordId: string,
  contextId: string | null,
) => useQuery(resolvedRecordPreviewOptions(recordId, contextId));
