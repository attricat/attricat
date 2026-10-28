import { Alert, Box, CircularProgress, Stack, Typography } from '@mui/material';
import { useQuery } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import { activeRunStatuses, runStatuses } from '../../api/runStatus';
import { ApiErrorAlert } from '../../components/ApiErrorAlert';
import { progressSizes } from '../../components/iconSizes';
import { RecordIdLink } from '../../components/RecordIdLink';
import { useRecordLabels } from '../records/useRecordLabels';
import { ruleRunOptions } from './queryOptions';

/**
 * Follows a dry run until it finishes, then says how many records it checked
 * and would flag, and names the first of them with what they would report.
 * `outdated` replaces the result when what was dry-run has changed since.
 */
export const DryRunResult = ({
  runId,
  outdated,
}: {
  runId: string;
  outdated?: string;
}) => {
  const { t } = useTranslation();
  const run = useQuery(ruleRunOptions(runId));
  const failures = run.data?.dry_run_failures ?? [];
  const labels = useRecordLabels(failures.map((failure) => failure.record_id));
  if (run.error) return <ApiErrorAlert error={run.error} sx={{ mt: 2 }} />;
  if (!run.data || activeRunStatuses.includes(run.data.status))
    return (
      <Stack direction="row" spacing={1} sx={{ alignItems: 'center', mt: 2 }}>
        <CircularProgress
          aria-hidden
          enableTrackSlot
          size={progressSizes.compact}
        />
        <Typography color="text.secondary" role="status" variant="body2">
          {t('rules.dryRunResult.checking')}
        </Typography>
      </Stack>
    );
  if (run.data.status !== runStatuses.completed)
    return (
      <Alert severity="error" sx={{ mt: 2 }}>
        {t('rules.dryRunResult.failed')}
      </Alert>
    );
  if (outdated)
    return (
      <Alert severity="info" sx={{ mt: 2 }}>
        {outdated}
      </Alert>
    );
  const count = run.data.findings_created;
  return (
    <Box sx={{ mt: 2 }}>
      <Alert
        role="status"
        severity={count > 0 || run.data.truncated ? 'warning' : 'success'}
      >
        {t('rules.dryRunResult.result', {
          count,
          evaluated: run.data.candidates_evaluated,
        })}
        {run.data.truncated && ` ${t('rules.dryRunResult.truncated')}`}
      </Alert>
      {failures.length > 0 && (
        <Box component="section" sx={{ mt: 2 }}>
          <Typography component="h3" variant="subtitle2">
            {t('rules.dryRunResult.failures')}
          </Typography>
          <Box component="ul" sx={{ listStyle: 'none', m: 0, p: 0 }}>
            {failures.map((failure, index) => (
              <Box
                component="li"
                // A record can fail in more than one way.
                key={`${failure.record_id}-${index}`}
                sx={{
                  py: 1,
                  '&:not(:last-of-type)': {
                    borderBottom: 1,
                    borderColor: 'divider',
                  },
                }}
              >
                <Typography variant="body2">
                  <RecordIdLink
                    label={labels.get(failure.record_id)}
                    recordId={failure.record_id}
                  />
                </Typography>
                <Typography color="text.secondary" variant="body2">
                  {failure.message}
                </Typography>
              </Box>
            ))}
          </Box>
          {count > failures.length && (
            <Typography color="text.secondary" variant="caption">
              {t('rules.dryRunResult.showingFirst', {
                count,
                shown: failures.length,
              })}
            </Typography>
          )}
        </Box>
      )}
    </Box>
  );
};
