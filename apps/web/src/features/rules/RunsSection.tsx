import {
  Box,
  Paper,
  Stack,
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableRow,
  Typography,
} from '@mui/material';
import type { ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { QueryErrorNotice } from '../../components/QueryErrorNotice';
import { Timestamp } from '../../time/Timestamp';
import type { Rule, RuleRun, RunsSearch } from './api';
import { RuleListFilter } from './RuleListFilter';
import { ruleFilterOptions } from './ruleFilterOptions';
import { RecordIdLink } from '../../components/RecordIdLink';
import { RuleRevisionCell } from './RuleCells';
import type { RulesById } from './rulesById';
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
  rulesById,
  runs,
  title,
  action,
  filters,
  showRule = true,
}: {
  /** Leaves out the rule column where every run is of one rule. */
  showRule?: boolean;
  /** The run history's filters and the rules they offer. */
  filters?: {
    rules: readonly Rule[];
    search: RunsSearch;
    onChange: (search: RunsSearch) => void;
  };
  /** Replaces the run history heading, as where only recent runs show. */
  title?: string;
  /** Beside the heading, such as a link to every run. */
  action?: ReactNode;
  recordLabels: ReadonlyMap<string, string>;
  /** The section's load failure, offered for retry. */
  error: Error | null;
  onRetry: () => void;
  rulesById: RulesById;
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
          <Stack
            direction="row"
            spacing={2}
            sx={{ alignItems: 'center', justifyContent: 'space-between' }}
          >
            <Typography component="h2" variant="h6">
              {title ?? t('rules.runHistory')}
            </Typography>
            {action}
          </Stack>
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
        {filters && (
          <Stack
            aria-label={t('rules.filters.runsLabel')}
            component="search"
            direction="row"
            spacing={2}
            useFlexGap
            sx={{ flexWrap: 'wrap', px: 2, pb: 2 }}
          >
            <RuleListFilter
              anyLabel={t('rules.filters.any')}
              label={t('rules.columns.rule')}
              onChange={(rule) => filters.onChange({ ...filters.search, rule })}
              options={ruleFilterOptions(filters.rules)}
              value={filters.search.rule}
            />
          </Stack>
        )}
        <Table size="small">
          <TableHead>
            <TableRow>
              <TableCell>{t('rules.columns.executed')}</TableCell>
              {showRule && <TableCell>{t('rules.columns.rule')}</TableCell>}
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
                {showRule && (
                  <TableCell>
                    <RuleRevisionCell
                      rules={rulesById}
                      ruleId={item.rule_id}
                      version={item.rule_version}
                    />
                  </TableCell>
                )}
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
                <TableCell>
                  {item.candidates_skipped > 0
                    ? t('rules.evaluatedWithSkipped', {
                        evaluated: item.candidates_evaluated,
                        skipped: item.candidates_skipped,
                      })
                    : item.candidates_evaluated}
                </TableCell>
                <TableCell>{item.findings_created}</TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      </Paper>
    </>
  );
};
