import { FindingSeverityChip } from './FindingSeverityChip';
import {
  Box,
  Button,
  FormControl,
  InputLabel,
  MenuItem,
  Paper,
  Select,
  Stack,
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableRow,
} from '@mui/material';
import { useId } from 'react';
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
import {
  ANY_FINDING_FILTER,
  FINDING_FILTER_MIN_WIDTH,
  findingStates,
  findingSeverities,
  findingStateFilters,
  findingsTableColumnCount,
} from './constants';

type FilterOption = { value: string; label: string };

/** One filter of the findings list; the empty value matches every finding. */
const FindingFilter = ({
  label,
  value,
  options,
  anyLabel,
  onChange,
}: {
  label: string;
  value: string | undefined;
  options: readonly FilterOption[];
  /** The label of the unset value, when the filter can be unset. */
  anyLabel?: string;
  onChange: (value: string | undefined) => void;
}) => {
  const labelId = useId();
  return (
    <FormControl size="small" sx={{ minWidth: FINDING_FILTER_MIN_WIDTH }}>
      <InputLabel id={labelId} shrink>
        {label}
      </InputLabel>
      <Select
        displayEmpty
        label={label}
        labelId={labelId}
        notched
        onChange={(event) =>
          onChange(
            event.target.value === ANY_FINDING_FILTER
              ? undefined
              : event.target.value,
          )
        }
        value={value ?? ANY_FINDING_FILTER}
      >
        {anyLabel !== undefined && (
          <MenuItem value={ANY_FINDING_FILTER}>{anyLabel}</MenuItem>
        )}
        {options.map((option) => (
          <MenuItem key={option.value} value={option.value}>
            {option.label}
          </MenuItem>
        ))}
      </Select>
    </FormControl>
  );
};

/** Each rule, by name. */
const ruleOptions = (rules: readonly Rule[]): FilterOption[] =>
  rules
    .map((rule) => ({ value: rule.id, label: rule.name }))
    .sort((a, b) => a.label.localeCompare(b.label));

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
          <FindingFilter
            label={t('rules.columns.state')}
            onChange={setFilter('state')}
            options={Object.values(findingStateFilters).map((state) => ({
              value: state,
              label: t(`rules.filters.states.${state}`),
            }))}
            value={filters.state ?? findingStateFilters.unresolved}
          />
          <FindingFilter
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
          <FindingFilter
            anyLabel={any}
            label={t('rules.columns.rule')}
            onChange={setFilter('rule')}
            options={ruleOptions(rules)}
            value={filters.rule}
          />
          <FindingFilter
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
          <FindingFilter
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
