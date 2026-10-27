import {
  Box,
  Paper,
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableRow,
  Typography,
} from '@mui/material';
import { useTranslation } from 'react-i18next';
import { QueryErrorNotice } from '../../components/QueryErrorNotice';
import { Timestamp } from '../../time/Timestamp';
import type { RuleRun } from './api';
import { RecordIdLink } from '../../components/RecordIdLink';
import { RuleRevisionCell } from './RuleCells';
import type { RuleRevisions } from './ruleRevisions';
import { RunStatusChip } from '../../components/RunStatusChip';

/** When a run finished, or when it was queued while it has not. */
const runTime = (run: RuleRun) => run.completed_at ?? run.created_at;

/** The most recent run time, or `undefined` before the first run. */
const lastRunTime = (runs: readonly RuleRun[]) =>
  runs
    .map(runTime)
    .reduce<string | undefined>(
      (latest, time) =>
        latest === undefined || Date.parse(time) > Date.parse(latest)
          ? time
          : latest,
      undefined,
    );

export const RunsSection = ({
  recordLabels,
  error,
  onRetry,
  revisions,
  runs,
}: {
  recordLabels: ReadonlyMap<string, string>;
  /** The section's load failure, offered for retry. */
  error: Error | null;
  onRetry: () => void;
  revisions: RuleRevisions;
  runs: readonly RuleRun[] | undefined;
}) => {
  const { t } = useTranslation();
  const lastRun = lastRunTime(runs ?? []);
  return (
    <>
      {error && (
        <Box sx={{ mt: 3 }}>
          <QueryErrorNotice error={error} onRetry={onRetry} />
        </Box>
      )}
      <Paper component="section" sx={{ mt: 3 }}>
        <Box sx={{ p: 2 }}>
          <Typography variant="h6">{t('rules.runHistory')}</Typography>
          {runs && (
            <Typography color="text.secondary" variant="body2">
              {t('rules.lastExecuted')}:{' '}
              <Timestamp
                fallback={t('rules.neverExecuted')}
                relative
                value={lastRun}
              />
            </Typography>
          )}
        </Box>
        <Table size="small">
          <TableHead>
            <TableRow>
              <TableCell>{t('rules.columns.executed')}</TableCell>
              <TableCell>{t('rules.columns.rule')}</TableCell>
              <TableCell>{t('rules.columns.scope')}</TableCell>
              <TableCell>{t('rules.columns.source')}</TableCell>
              <TableCell>{t('rules.columns.status')}</TableCell>
              <TableCell>{t('rules.columns.evaluated')}</TableCell>
              <TableCell>{t('rules.columns.findings')}</TableCell>
            </TableRow>
          </TableHead>
          <TableBody>
            {(runs ?? []).map((item) => (
              <TableRow key={item.id}>
                <TableCell>
                  <Timestamp relative value={runTime(item)} />
                </TableCell>
                <TableCell>
                  <RuleRevisionCell
                    revisions={revisions}
                    ruleId={item.rule_id}
                    version={item.rule_version}
                  />
                </TableCell>
                <TableCell>
                  {item.scope_record_id ? (
                    <RecordIdLink
                      recordId={item.scope_record_id}
                      label={recordLabels.get(item.scope_record_id)}
                    />
                  ) : (
                    t('rules.allRecords')
                  )}
                </TableCell>
                <TableCell>
                  {item.dry_run
                    ? t('rules.dryRunSuffix', {
                        source: t(`rules.runSources.${item.source}`),
                      })
                    : t(`rules.runSources.${item.source}`)}
                </TableCell>
                <TableCell>
                  <RunStatusChip status={item.status} />
                </TableCell>
                <TableCell>{item.candidates_evaluated}</TableCell>
                <TableCell>{item.findings_created}</TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      </Paper>
    </>
  );
};
