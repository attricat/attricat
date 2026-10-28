import { useAcknowledgeFinding } from './useAcknowledgeFinding';
import {
  useInfiniteQuery,
  useMutation,
  useQuery,
  useQueryClient,
} from '@tanstack/react-query';
import { createLink, useNavigate } from '@tanstack/react-router';
import {
  Box,
  Button,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  Tab,
  Tabs,
} from '@mui/material';
import { useState } from 'react';
import { PlusIcon } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { ApiErrorAlert } from '../../components/ApiErrorAlert';
import { PageContainer } from '../../components/PageContainer';
import { PageHeader } from '../../components/PageHeader';
import { RouterButton } from '../../components/RouterLink';
import { toast } from '../../components/toast';
import { DryRunResult } from './DryRunResult';
import { useCan } from '../auth/useCan';
import {
  listFindingsPage,
  runRuleNow,
  type FindingsSearch,
  type Rule,
} from './api';
import {
  RULE_SECTION_FINDINGS,
  RULE_SECTION_RULES,
  RULE_SECTION_RUNS,
  RULE_SECTIONS,
  ruleRoutes,
  type RuleInspectionSection,
} from './constants';
import { FindingsSection } from './FindingsSection';
import { ruleQueryKeys } from './queryKeys';
import { ruleDefinitionsOptions, ruleRunsOptions } from './queryOptions';
import { rulesById } from './rulesById';
import { useRecordLabels } from '../records/useRecordLabels';
import { RulesSection } from './RulesSection';
import { RuleToggleError } from './RuleToggleError';
import { useToggleRule } from './useToggleRule';
import { RunsSection } from './RunsSection';
import {
  RuleFindingIcon,
  RuleIcon,
  RuleRunIcon,
} from '../../components/systemIcons';
import { compactIconSize } from '../../components/iconSizes';
import { useTabAccessibility } from '../../components/useTabAccessibility';
import type { LucideIcon } from 'lucide-react';
import { capabilities } from '../auth/capabilities';
import { blueprintCatalogueOptions } from '../blueprints/queryOptions';
import { contextListOptions } from '../contexts/queryOptions';

const RouterTab = createLink(Tab);
const sectionIcons = {
  [RULE_SECTION_RULES]: RuleIcon,
  [RULE_SECTION_FINDINGS]: RuleFindingIcon,
  [RULE_SECTION_RUNS]: RuleRunIcon,
} satisfies Record<RuleInspectionSection, LucideIcon>;

const noFindingsSearch: FindingsSearch = {};

export const RuleInspectionPage = ({
  section,
  findingsSearch = noFindingsSearch,
}: {
  section: RuleInspectionSection;
  /** The findings section's filters, from its URL. */
  findingsSearch?: FindingsSearch;
}) => {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const canManage = useCan(capabilities.rulesManage);
  // Findings and runs name their rule from the definitions.
  const rules = useQuery(ruleDefinitionsOptions());
  const rulesByIdMap = rulesById(rules.data ?? []);
  const findingsShown = section === RULE_SECTION_FINDINGS;
  const findings = useInfiniteQuery({
    queryKey: ruleQueryKeys.findingsPage(findingsSearch),
    queryFn: ({ pageParam }) => listFindingsPage(findingsSearch, pageParam),
    initialPageParam: 0,
    getNextPageParam: (page) => page.nextOffset,
    enabled: findingsShown,
  });
  const findingItems = findings.data?.pages.flatMap((page) => page.items);
  // Rules name their blueprint, and the findings filters offer the
  // workspace's blueprints and contexts.
  const blueprints = useQuery({
    ...blueprintCatalogueOptions(),
    enabled: findingsShown || section === RULE_SECTION_RULES,
  });
  const contexts = useQuery({
    ...contextListOptions(),
    enabled: findingsShown,
  });
  const runs = useQuery({
    ...ruleRunsOptions(),
    enabled: section === RULE_SECTION_RUNS,
  });
  // Name the records the visible findings and runs refer to.
  const recordLabels = useRecordLabels(
    findingsShown
      ? (findingItems ?? []).map((finding) => finding.record_id)
      : section === RULE_SECTION_RUNS
        ? (runs.data ?? []).flatMap((run) =>
            run.scope_record_id ? [run.scope_record_id] : [],
          )
        : [],
  );
  const refresh = () =>
    queryClient.invalidateQueries({ queryKey: ruleQueryKeys.all });
  const acknowledge = useAcknowledgeFinding();
  // A dry run's result opens in a dialog; a run reports to the run history.
  const [dryRun, setDryRun] = useState<{ name: string; runId: string }>();
  const run = useMutation({
    mutationFn: ({ rule, dryRun }: { rule: Rule; dryRun: boolean }) =>
      runRuleNow(rule.id, dryRun),
    meta: { toast: false },
    onSuccess: (started, { rule, dryRun }) => {
      if (dryRun) setDryRun({ name: rule.name, runId: started.id });
      else toast.success(t('rules.runStarted'));
      return refresh();
    },
  });
  const toggle = useToggleRule();

  const tabId = useTabAccessibility();
  const activeSectionIndex = RULE_SECTIONS.findIndex(
    (item) => item.key === section,
  );

  return (
    <PageContainer>
      <PageHeader
        icon={RuleIcon}
        title={t('rules.title')}
        description={t('rules.description')}
        actions={
          canManage && (
            <RouterButton
              startIcon={<PlusIcon />}
              to={ruleRoutes.create}
              variant="contained"
            >
              {t('rules.newRule')}
            </RouterButton>
          )
        }
      />
      <Tabs
        aria-label={t('rules.inspectionTabs')}
        sx={{ mt: 3 }}
        value={activeSectionIndex}
      >
        {RULE_SECTIONS.map((item, index) => {
          const Icon = sectionIcons[item.key];
          return (
            <RouterTab
              {...tabId.tab(index)}
              icon={<Icon aria-hidden size={compactIconSize} />}
              key={item.to}
              label={t(`rules.sections.${item.key}`)}
              to={item.to}
              value={index}
            />
          );
        })}
      </Tabs>
      <Box {...tabId.panel(activeSectionIndex)}>
        {section === RULE_SECTION_RULES && run.error && (
          <ApiErrorAlert error={run.error} sx={{ mt: 3 }} />
        )}
        {section === RULE_SECTION_RULES && (
          <RuleToggleError sx={{ mt: 3 }} toggle={toggle} />
        )}
        {section === RULE_SECTION_FINDINGS && acknowledge.error && (
          <ApiErrorAlert error={acknowledge.error} sx={{ mt: 3 }} />
        )}
        {section === RULE_SECTION_RULES && (
          <RulesSection
            blueprints={blueprints.data ?? []}
            canManage={canManage}
            error={rules.error}
            onRetry={() => void rules.refetch()}
            onRun={(rule, dryRun) => {
              toggle.reset();
              run.mutate({ rule, dryRun });
            }}
            onToggle={(rule, enabled) => {
              run.reset();
              toggle.mutate({ rule, enabled });
            }}
            running={run.isPending || toggle.isPending}
            rules={rules.data}
          />
        )}
        {section === RULE_SECTION_FINDINGS && (
          <FindingsSection
            onAcknowledge={(id) => acknowledge.mutate(id)}
            acknowledging={acknowledge.isPending}
            canManage={canManage}
            error={findings.isFetchNextPageError ? null : findings.error}
            onRetry={() => void findings.refetch()}
            recordLabels={recordLabels}
            findings={findingItems}
            filters={findingsSearch}
            onFiltersChange={(search) =>
              void navigate({ to: ruleRoutes.findings, search })
            }
            rules={rules.data ?? []}
            blueprints={blueprints.data ?? []}
            contexts={contexts.data ?? []}
            hasNextPage={findings.hasNextPage}
            isFetchingNextPage={findings.isFetchingNextPage}
            nextPageError={
              findings.isFetchNextPageError ? findings.error : null
            }
            onLoadMore={() => void findings.fetchNextPage()}
            rulesById={rulesByIdMap}
          />
        )}
        {section === RULE_SECTION_RUNS && (
          <RunsSection
            recordLabels={recordLabels}
            error={runs.error}
            onRetry={() => void runs.refetch()}
            rulesById={rulesByIdMap}
            runs={runs.data}
          />
        )}
      </Box>
      <Dialog
        fullWidth
        maxWidth="sm"
        onClose={() => setDryRun(undefined)}
        open={dryRun !== undefined}
      >
        <DialogTitle>
          {dryRun && t('rules.dryRunResult.title', { name: dryRun.name })}
        </DialogTitle>
        <DialogContent>
          {dryRun && <DryRunResult runId={dryRun.runId} />}
        </DialogContent>
        <DialogActions>
          <Button onClick={() => setDryRun(undefined)}>
            {t('common.close')}
          </Button>
        </DialogActions>
      </Dialog>
    </PageContainer>
  );
};
