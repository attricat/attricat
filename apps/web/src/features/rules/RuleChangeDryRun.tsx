import { Alert, Button, CircularProgress } from '@mui/material';
import { useMutation, useQuery } from '@tanstack/react-query';
import { FlaskConicalIcon } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { activeRunStatuses, runStatuses } from '../../api/runStatus';
import { ApiErrorAlert } from '../../components/ApiErrorAlert';
import { progressSizes } from '../../components/iconSizes';
import { runRuleNow } from './api';
import { RULE_DRY_RUN_POLL_MS } from './constants';
import { ruleRunsOptions } from './queryOptions';

/**
 * Dry-runs an unsaved definition of a rule and reports how many records it
 * would flag, so an enforcing change can be checked before it is saved.
 */
export const RuleChangeDryRun = ({
  definition,
  disabled,
  ruleId,
}: {
  definition: string;
  disabled: boolean;
  ruleId: string;
}) => {
  const { t } = useTranslation();
  const start = useMutation({
    mutationFn: (source: string) => runRuleNow(ruleId, true, source),
  });
  const runId = start.data?.id;
  const runs = useQuery({
    ...ruleRunsOptions(),
    enabled: runId !== undefined,
    refetchInterval: (query) => {
      const run = query.state.data?.find((item) => item.id === runId);
      return !run || activeRunStatuses.includes(run.status)
        ? RULE_DRY_RUN_POLL_MS
        : false;
    },
  });
  const run = runs.data?.find((item) => item.id === runId);
  // The run checked the definition as it was when it started.
  const checked = start.variables === definition;
  const running =
    start.isPending ||
    (runId !== undefined && (!run || activeRunStatuses.includes(run.status)));
  return (
    <>
      <Button
        disabled={disabled || running}
        onClick={() => start.mutate(definition)}
        startIcon={
          running ? (
            <CircularProgress size={progressSizes.compact} />
          ) : (
            <FlaskConicalIcon />
          )
        }
        variant="outlined"
      >
        {t('rules.editor.dryRunChange')}
      </Button>
      <ApiErrorAlert error={start.error ?? runs.error} sx={{ mt: 2 }} />
      {run?.status === runStatuses.completed && (
        <Alert
          severity={run.findings_created > 0 ? 'warning' : 'success'}
          sx={{ mt: 2 }}
        >
          {checked
            ? t('rules.editor.dryRunResult', {
                count: run.findings_created,
                evaluated: run.candidates_evaluated,
              })
            : t('rules.editor.dryRunOutdated')}
        </Alert>
      )}
      {(run?.status === runStatuses.deadLetter ||
        run?.status === runStatuses.cancelled) && (
        <Alert severity="error" sx={{ mt: 2 }}>
          {t('rules.editor.dryRunFailed')}
        </Alert>
      )}
    </>
  );
};
