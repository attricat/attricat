import { Button, CircularProgress } from '@mui/material';
import { useMutation, useQuery } from '@tanstack/react-query';
import { FlaskConicalIcon } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { activeRunStatuses } from '../../api/runStatus';
import { ApiErrorAlert } from '../../components/ApiErrorAlert';
import { progressSizes } from '../../components/iconSizes';
import { runRuleNow } from './api';
import { DryRunResult } from './DryRunResult';
import { ruleRunOptions } from './queryOptions';

/**
 * Dry-runs an unsaved definition of a rule and reports which records it
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
    meta: { toast: false },
  });
  const runId = start.data?.id;
  const run = useQuery({
    ...ruleRunOptions(runId ?? ''),
    enabled: runId !== undefined,
  });
  // The run checked the definition as it was when it started.
  const checked = start.variables === definition;
  const running =
    start.isPending ||
    (runId !== undefined &&
      (!run.data || activeRunStatuses.includes(run.data.status)));
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
      <ApiErrorAlert error={start.error} sx={{ mt: 2 }} />
      {runId && (
        <DryRunResult
          outdated={checked ? undefined : t('rules.editor.dryRunOutdated')}
          runId={runId}
        />
      )}
    </>
  );
};
