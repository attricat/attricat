import { useMutation, useQueryClient } from '@tanstack/react-query';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { toast } from '../../components/toast';
import { runRuleNow, type Rule } from './api';
import { ruleQueryKeys } from './queryKeys';

/** A dry run whose result is shown: the rule's name and the run. */
export type ShownDryRun = { blueprintId: string; name: string; runId: string };

/**
 * Starts a run or dry run of a rule's current revision. A dry run's result
 * is kept to show; a run reports to the run history.
 */
export const useRunRule = () => {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const [dryRun, setDryRun] = useState<ShownDryRun>();
  const run = useMutation({
    mutationFn: ({ rule, dryRun }: { rule: Rule; dryRun: boolean }) =>
      runRuleNow(rule.id, dryRun),
    meta: { toast: false },
    onSuccess: (started, { rule, dryRun }) => {
      if (dryRun)
        setDryRun({
          blueprintId: rule.blueprint_id,
          name: rule.name,
          runId: started.id,
        });
      else toast.success(t('rules.runStarted'));
      return queryClient.invalidateQueries({ queryKey: ruleQueryKeys.all });
    },
  });
  return { run, dryRun, closeDryRun: () => setDryRun(undefined) };
};
