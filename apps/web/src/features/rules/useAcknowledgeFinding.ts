import { useMutation, useQueryClient } from '@tanstack/react-query';
import { acknowledgeFinding } from './api';
import { ruleQueryKeys } from './queryKeys';
import { refreshRecordRuleState } from './refreshRecordRuleState';

/**
 * Acknowledges a finding, then refreshes every list and count of findings and
 * the record's tags and data quality, which the server updates with it.
 */
export const useAcknowledgeFinding = () => {
  const client = useQueryClient();
  return useMutation({
    mutationFn: acknowledgeFinding,
    onSuccess: (finding) =>
      Promise.all([
        client.invalidateQueries({ queryKey: ruleQueryKeys.findings() }),
        refreshRecordRuleState(client, finding.record_id),
      ]),
  });
};
