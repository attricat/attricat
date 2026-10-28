import { FindingSeverityChip } from './FindingSeverityChip';
import {
  Box,
  Button,
  Checkbox,
  Paper,
  Stack,
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableRow,
  Typography,
  useMediaQuery,
  useTheme,
} from '@mui/material';
import { CheckCheckIcon, CheckIcon } from 'lucide-react';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { LoadingState } from '../../components/LoadingState';
import { ConfirmDialog } from '../../components/ConfirmDialog';
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
  total,
  openMatching,
  bulk,
}: {
  /** How many findings the filters match, once counted. */
  total: number | undefined;
  /** How many of them are open, which acknowledging all would change. */
  openMatching: number | undefined;
  /**
   * Acknowledges many findings: those listed, or all open ones matching.
   * Each resolves to whether it succeeded.
   */
  bulk: {
    acknowledgeSelected: (ids: readonly string[]) => Promise<boolean>;
    acknowledgeMatching: () => Promise<boolean>;
    isPending: boolean;
    error: Error | null;
  };
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
  const theme = useTheme();
  // Narrow screens stack each finding instead of squeezing a table.
  const narrow = useMediaQuery(theme.breakpoints.down('sm'));
  const messageOf = useFindingMessages(findings ?? [], rulesById);
  const any = t('rules.filters.any');
  const [selected, setSelected] = useState<ReadonlySet<string>>(new Set());
  const [confirmAll, setConfirmAll] = useState(false);
  // Selection is of the findings listed now.
  const setFilter =
    (key: keyof FindingsSearch) => (value: string | undefined) => {
      setSelected(new Set());
      onFiltersChange({ ...filters, [key]: value });
    };
  const selectable = (findings ?? []).filter(
    (finding) => finding.state === findingStates.open,
  );
  const selectedIds = selectable
    .map((finding) => finding.id)
    .filter((id) => selected.has(id));
  const toggle = (id: string) =>
    setSelected((current) => {
      const next = new Set(current);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  const allSelected =
    selectable.length > 0 && selectedIds.length === selectable.length;
  const selectBox = (finding: Finding) =>
    canManage &&
    finding.state === findingStates.open && (
      <Checkbox
        checked={selected.has(finding.id)}
        onChange={() => toggle(finding.id)}
        slotProps={{
          input: {
            'aria-label': t('rules.selectFinding', {
              record: recordLabels.get(finding.record_id) ?? finding.record_id,
              message: messageOf(finding),
            }),
          },
        }}
      />
    );
  const ruleOf = (finding: Finding) => (
    <RuleRevisionCell
      rules={rulesById}
      ruleId={finding.rule_id}
      version={finding.rule_version}
    />
  );
  const recordOf = (finding: Finding) => (
    <RecordIdLink
      recordId={finding.record_id}
      label={recordLabels.get(finding.record_id)}
    />
  );
  const stateOf = (finding: Finding) =>
    t(`rules.findingStates.${finding.state}`, { defaultValue: finding.state });
  const acknowledgeButton = (finding: Finding) =>
    canManage &&
    finding.state === findingStates.open && (
      <Button
        disabled={acknowledging}
        size="small"
        onClick={() => onAcknowledge(finding.id)}
        startIcon={<CheckIcon />}
      >
        {t('rules.acknowledge')}
      </Button>
    );
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
        <Stack
          direction="row"
          spacing={1}
          useFlexGap
          sx={{ alignItems: 'center', flexWrap: 'wrap', px: 2, pb: 1 }}
        >
          <Typography
            color="text.secondary"
            role="status"
            sx={{ flexGrow: 1 }}
            variant="body2"
          >
            {total !== undefined && t('rules.findingsCount', { count: total })}
          </Typography>
          {canManage && (
            <>
              <Button
                disabled={selectedIds.length === 0 || bulk.isPending}
                onClick={() =>
                  void bulk
                    .acknowledgeSelected(selectedIds)
                    .then((done) => done && setSelected(new Set()))
                }
                size="small"
                startIcon={<CheckCheckIcon />}
              >
                {t('rules.acknowledgeSelected', { count: selectedIds.length })}
              </Button>
              {openMatching !== undefined && openMatching > 0 && (
                <Button
                  disabled={bulk.isPending}
                  onClick={() => setConfirmAll(true)}
                  size="small"
                  startIcon={<CheckCheckIcon />}
                >
                  {t('rules.acknowledgeAllMatching', { count: openMatching })}
                </Button>
              )}
            </>
          )}
        </Stack>
        {!confirmAll && (
          <ApiErrorAlert error={bulk.error} sx={{ mx: 2, mb: 1 }} />
        )}
        {findings === undefined && !error && (
          <LoadingState sx={{ px: 2, pb: 2 }}>
            {t('rules.loadingFindings')}
          </LoadingState>
        )}
        {narrow ? (
          <Box component="ul" sx={{ listStyle: 'none', m: 0, p: 0 }}>
            {findings?.length === 0 && (
              <Typography
                component="li"
                color="text.secondary"
                sx={{ px: 2, py: 1.5 }}
                variant="body2"
              >
                {t('rules.filters.noMatches')}
              </Typography>
            )}
            {(findings ?? []).map((finding) => (
              <Box
                component="li"
                key={finding.id}
                sx={{
                  borderTop: 1,
                  borderColor: 'divider',
                  display: 'flex',
                  gap: 1,
                  px: 2,
                  py: 1.5,
                  ...(selected.has(finding.id) && {
                    bgcolor: 'action.selected',
                  }),
                }}
              >
                {canManage && <Box sx={{ mt: -1 }}>{selectBox(finding)}</Box>}
                <Stack spacing={0.5} sx={{ flex: 1, minWidth: 0 }}>
                  <Stack
                    direction="row"
                    spacing={1}
                    sx={{ alignItems: 'center', flexWrap: 'wrap' }}
                  >
                    <FindingSeverityChip severity={finding.severity} />
                    <Typography color="text.secondary" variant="caption">
                      {stateOf(finding)}
                    </Typography>
                  </Stack>
                  <Typography variant="body2">{messageOf(finding)}</Typography>
                  <Typography variant="body2">{recordOf(finding)}</Typography>
                  <Typography variant="caption">{ruleOf(finding)}</Typography>
                  {acknowledgeButton(finding) && (
                    <Box>{acknowledgeButton(finding)}</Box>
                  )}
                </Stack>
              </Box>
            ))}
          </Box>
        ) : (
          <Table size="small">
            <TableHead>
              <TableRow>
                {canManage && (
                  <TableCell padding="checkbox">
                    <Checkbox
                      checked={allSelected}
                      disabled={selectable.length === 0}
                      indeterminate={selectedIds.length > 0 && !allSelected}
                      onChange={() =>
                        setSelected(
                          allSelected
                            ? new Set()
                            : new Set(selectable.map((finding) => finding.id)),
                        )
                      }
                      slotProps={{
                        input: { 'aria-label': t('rules.selectAllFindings') },
                      }}
                    />
                  </TableCell>
                )}
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
                    colSpan={findingsTableColumnCount + (canManage ? 1 : 0)}
                    sx={{ color: 'text.secondary' }}
                  >
                    {t('rules.filters.noMatches')}
                  </TableCell>
                </TableRow>
              )}
              {(findings ?? []).map((finding) => (
                <TableRow key={finding.id} selected={selected.has(finding.id)}>
                  {canManage && (
                    <TableCell padding="checkbox">
                      {selectBox(finding)}
                    </TableCell>
                  )}
                  <TableCell>
                    <FindingSeverityChip severity={finding.severity} />
                  </TableCell>
                  <TableCell>{messageOf(finding)}</TableCell>
                  <TableCell>{ruleOf(finding)}</TableCell>
                  <TableCell>{recordOf(finding)}</TableCell>
                  <TableCell>{stateOf(finding)}</TableCell>
                  <TableCell>{acknowledgeButton(finding)}</TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        )}
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
      {confirmAll && openMatching !== undefined && (
        <ConfirmDialog
          confirmLabel={t('rules.acknowledgeAllMatching', {
            count: openMatching,
          })}
          error={bulk.error}
          isPending={bulk.isPending}
          onClose={() => setConfirmAll(false)}
          onConfirm={() =>
            void bulk.acknowledgeMatching().then((done) => {
              if (!done) return;
              setSelected(new Set());
              setConfirmAll(false);
            })
          }
          title={t('rules.acknowledgeAllTitle')}
        >
          {t('rules.acknowledgeAllConfirm', { count: openMatching })}
        </ConfirmDialog>
      )}
    </>
  );
};
