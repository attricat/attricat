import { Button, CircularProgress, Stack } from '@mui/material';
import { useMutation, useQuery } from '@tanstack/react-query';
import { FlaskConicalIcon } from 'lucide-react';
import type { ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { ApiErrorAlert } from '../../components/ApiErrorAlert';
import { progressSizes } from '../../components/iconSizes';
import { runRuleNow } from './api';
import { DryRunResult } from './DryRunResult';
import { ruleRunOptions } from './queryOptions';
import { isActiveRun } from './runCompletion';

/**
 * Dry-runs an unsaved definition of a rule and reports which records it
 * would flag, so an enforcing change can be checked before it is saved. The
 * dry-run button sits beside `heading`, and the result below them.
 */
export const RuleChangeDryRun = ({
  blueprintId,
  definition,
  disabled,
  heading,
  ruleId,
}: {
  /** The rule's blueprint, to name the attributes its findings mention. */
  blueprintId: string;
  definition: string;
  disabled: boolean;
  heading: ReactNode;
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
      !run.isError &&
      (!run.data || isActiveRun(run.data)));
  return (
    <>
      <Stack
        direction={{ xs: 'column', sm: 'row' }}
        spacing={2}
        sx={{
          alignItems: { sm: 'center' },
          justifyContent: 'space-between',
        }}
      >
        {heading}
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
      </Stack>
      <ApiErrorAlert error={start.error} sx={{ mt: 2 }} />
      {runId && (
        <DryRunResult
          blueprintId={blueprintId}
          outdated={checked ? undefined : t('rules.editor.dryRunOutdated')}
          runId={runId}
        />
      )}
    </>
  );
};
