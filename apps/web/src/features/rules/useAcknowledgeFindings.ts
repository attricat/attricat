import { useMutation, useQueryClient } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import { toast } from '../../components/toast';
import { recordQueryKeys } from '../records/queryKeys';
import { acknowledgeFindings } from './api';
import { ruleQueryKeys } from './queryKeys';

/**
 * Acknowledges many findings at once, then refreshes every list and count of
 * findings, the rules' counts and the record views, and marks record searches
 * stale, since the server updates the records' needs-attention tags with them.
 */
export const useAcknowledgeFindings = () => {
  const { t } = useTranslation();
  const client = useQueryClient();
  return useMutation({
    mutationFn: acknowledgeFindings,
    meta: { toast: false },
    onSuccess: ({ acknowledged }) => {
      toast.success(t('rules.bulkAcknowledged', { count: acknowledged }));
      return Promise.all([
        client.invalidateQueries({ queryKey: ruleQueryKeys.findings() }),
        client.invalidateQueries({ queryKey: ruleQueryKeys.definitions() }),
        // The acknowledged findings' records are not returned, so every
        // record view refreshes its needs-attention tag and data quality.
        ...[
          recordQueryKeys.forms(),
          recordQueryKeys.previews(),
          recordQueryKeys.allResolvedPreviews(),
        ].map((queryKey) => client.invalidateQueries({ queryKey })),
        client.invalidateQueries({
          queryKey: recordQueryKeys.searches(),
          refetchType: 'none',
        }),
      ]);
    },
  });
};
