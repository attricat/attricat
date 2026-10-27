import { useMutation, useQueryClient } from '@tanstack/react-query';
import { disableRule, enableRule, type Rule } from './api';
import { ruleQueryKeys } from './queryKeys';

export type ToggleRuleInput = {
  rule: Rule;
  enabled: boolean;
  /** Enable an enforcing rule although its dry run found violations. */
  acceptExistingViolations?: boolean;
};

/** Enables or disables a rule and refreshes every rule query. */
export const useToggleRule = () => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({
      rule,
      enabled,
      acceptExistingViolations,
    }: ToggleRuleInput) =>
      enabled
        ? enableRule(rule.id, acceptExistingViolations)
        : disableRule(rule.id),
    onSuccess: () =>
      queryClient.invalidateQueries({ queryKey: ruleQueryKeys.all }),
  });
};

export type ToggleRule = ReturnType<typeof useToggleRule>;
