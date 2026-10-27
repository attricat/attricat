import { useMutation, useQueryClient } from '@tanstack/react-query';
import { acknowledgeFinding } from './api';
import { ruleQueryKeys } from './queryKeys';

/** Acknowledges a finding, then refreshes every list and count of findings. */
export const useAcknowledgeFinding = () => {
  const client = useQueryClient();
  return useMutation({
    mutationFn: acknowledgeFinding,
    onSuccess: () =>
      client.invalidateQueries({ queryKey: ruleQueryKeys.findings() }),
  });
};
