import { FindingSeverityChip } from './FindingSeverityChip';
import {
  Box,
  Button,
  Paper,
  Stack,
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableRow,
} from '@mui/material';
import { useTranslation } from 'react-i18next';
import { QueryErrorNotice } from '../../components/QueryErrorNotice';
import { ApiErrorAlert } from '../../components/ApiErrorAlert';
import { LoadMoreButton } from '../../components/LoadMoreButton';
import type { Blueprint } from '../blueprints/api';
import type { AttributeContext } from '../contexts/api';
import { blueprintKinds } from '../blueprints/constants';
import { contextLabel } from '../contexts/defaultContext';
import { lexiconText } from '../lexicon/lexicon';
import type { Finding, FindingsSearch, Rule } from './api';
import { RecordIdLink } from '../../components/RecordIdLink';
import { RuleRevisionCell } from './RuleCells';
import type { RulesById } from './rulesById';
import { useFindingMessages } from './useFindingMessages';
import { RuleListFilter } from './RuleListFilter';
import { ruleFilterOptions } from './ruleFilterOptions';
import {
  findingStates,
  findingSeverities,
  findingStateFilters,
  findingsTableColumnCount,
} from './constants';

export const FindingsSection = ({
  acknowledging,
  canManage,
  error,
  onRetry,
  findings,
  filters,
  onFiltersChange,
  rules,
  blueprints,
  contexts,
  hasNextPage,
  isFetchingNextPage,
  nextPageError,
  onLoadMore,
  recordLabels,
  onAcknowledge,
  rulesById,
}: {
  acknowledging: boolean;
  canManage: boolean;
  /** The section's load failure, offered for retry. */
  error: Error | null;
  onRetry: () => void;
  findings: readonly Finding[] | undefined;
  filters: FindingsSearch;
  onFiltersChange: (filters: FindingsSearch) => void;
  rules: readonly Rule[];
  blueprints: readonly Blueprint[];
  contexts: readonly AttributeContext[];
  hasNextPage: boolean;
  isFetchingNextPage: boolean;
  nextPageError: Error | null;
  onLoadMore: () => void;
  recordLabels: ReadonlyMap<string, string>;
  onAcknowledge: (id: string) => void;
  rulesById: RulesById;
}) => {
  const { t } = useTranslation();
  const messageOf = useFindingMessages(findings ?? [], rulesById);
  const any = t('rules.filters.any');
  const setFilter =
    (key: keyof FindingsSearch) => (value: string | undefined) =>
      onFiltersChange({ ...filters, [key]: value });
  return (
    <>
      {error && (
        <Box sx={{ mt: 3 }}>
          <QueryErrorNotice error={error} onRetry={onRetry} />
        </Box>
      )}
      <Paper component="section" sx={{ mt: 3 }}>
        <Stack
          aria-label={t('rules.filters.label')}
          component="search"
          direction="row"
          spacing={2}
          useFlexGap
          sx={{ flexWrap: 'wrap', p: 2 }}
        >
          <RuleListFilter
            label={t('rules.columns.state')}
            onChange={setFilter('state')}
            options={Object.values(findingStateFilters).map((state) => ({
              value: state,
              label: t(`rules.filters.states.${state}`),
            }))}
            value={filters.state ?? findingStateFilters.unresolved}
          />
          <RuleListFilter
            anyLabel={any}
            label={t('rules.columns.severity')}
            onChange={setFilter('severity')}
            options={[...Object.values(findingSeverities)]
              .reverse()
              .map((severity) => ({
                value: severity,
                label: t(`rules.severities.${severity}`),
              }))}
            value={filters.severity}
          />
          <RuleListFilter
            anyLabel={any}
            label={t('rules.columns.rule')}
            onChange={setFilter('rule')}
            options={ruleFilterOptions(rules)}
            value={filters.rule}
          />
          <RuleListFilter
            anyLabel={any}
            label={t('rules.filters.blueprint')}
            onChange={setFilter('blueprint')}
            options={blueprints
              .filter((blueprint) => blueprint.kind === blueprintKinds.record)
              .map((blueprint) => ({
                value: blueprint.id,
                label: lexiconText(blueprint.name),
              }))
              .sort((a, b) => a.label.localeCompare(b.label))}
            value={filters.blueprint}
          />
          <RuleListFilter
            anyLabel={any}
            label={t('rules.filters.context')}
            onChange={setFilter('context')}
            options={contexts.map((context) => ({
              value: context.id,
              label: contextLabel(t, context.code),
            }))}
            value={filters.context}
          />
        </Stack>
        <Table size="small">
          <TableHead>
            <TableRow>
              <TableCell>{t('rules.columns.severity')}</TableCell>
              <TableCell>{t('rules.columns.finding')}</TableCell>
              <TableCell>{t('rules.columns.rule')}</TableCell>
              <TableCell>{t('rules.columns.record')}</TableCell>
              <TableCell>{t('rules.columns.state')}</TableCell>
              <TableCell />
            </TableRow>
          </TableHead>
          <TableBody>
            {findings?.length === 0 && (
              <TableRow>
                <TableCell
                  colSpan={findingsTableColumnCount}
                  sx={{ color: 'text.secondary' }}
                >
                  {t('rules.filters.noMatches')}
                </TableCell>
              </TableRow>
            )}
            {(findings ?? []).map((finding) => (
              <TableRow key={finding.id}>
                <TableCell>
                  <FindingSeverityChip severity={finding.severity} />
                </TableCell>
                <TableCell>{messageOf(finding)}</TableCell>
                <TableCell>
                  <RuleRevisionCell
                    rules={rulesById}
                    ruleId={finding.rule_id}
                    version={finding.rule_version}
                  />
                </TableCell>
                <TableCell>
                  <RecordIdLink
                    recordId={finding.record_id}
                    label={recordLabels.get(finding.record_id)}
                  />
                </TableCell>
                <TableCell>
                  {t(`rules.findingStates.${finding.state}`, {
                    defaultValue: finding.state,
                  })}
                </TableCell>
                <TableCell>
                  {canManage && finding.state === findingStates.open && (
                    <Button
                      disabled={acknowledging}
                      size="small"
                      onClick={() => onAcknowledge(finding.id)}
                    >
                      {t('rules.acknowledge')}
                    </Button>
                  )}
                </TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
        {(hasNextPage || nextPageError) && (
          <Box sx={{ p: 2 }}>
            <ApiErrorAlert error={nextPageError} sx={{ mb: 2 }} />
            {hasNextPage && (
              <LoadMoreButton
                isLoading={isFetchingNextPage}
                onLoadMore={onLoadMore}
              />
            )}
          </Box>
        )}
      </Paper>
    </>
  );
};
