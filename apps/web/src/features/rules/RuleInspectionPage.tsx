import {
  useInfiniteQuery,
  useMutation,
  useQuery,
  useQueryClient,
} from '@tanstack/react-query';
import { createLink, useNavigate } from '@tanstack/react-router';
import { Alert, Box, Button, Tab, Tabs } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { ApiRequestError } from '../../api/request';
import { ApiErrorAlert } from '../../components/ApiErrorAlert';
import { PageContainer } from '../../components/PageContainer';
import { PageHeader } from '../../components/PageHeader';
import { useCan } from '../auth/useCan';
import {
  acknowledgeFinding,
  disableRule,
  enableRuleRevision,
  listFindingsPage,
  listRuleRuns,
  runRuleNow,
  type FindingsSearch,
  type Rule,
} from './api';
import {
  RULE_HAS_EXISTING_VIOLATIONS,
  RULE_SECTION_FINDINGS,
  RULE_SECTION_RULES,
  RULE_SECTION_RUNS,
  RULE_SECTIONS,
  RULE_TAB_ID_PREFIX,
  RULE_TABPANEL_ID_PREFIX,
  type RuleInspectionSection,
} from './constants';
import { FindingsSection } from './FindingsSection';
import { ruleQueryKeys } from './queryKeys';
import { ruleDefinitionsOptions } from './queryOptions';
import { ruleRevisionsByKey } from './ruleRevisions';
import { useRecordLabels } from '../records/useRecordLabels';
import { RulesSection } from './RulesSection';
import { RunsSection } from './RunsSection';
import { RuleIcon } from '../../components/systemIcons';
import { errorMessage } from '../../api/errors';
import { capabilities } from '../auth/capabilities';
import { blueprintCatalogueOptions } from '../blueprints/queryOptions';
import { contextListOptions } from '../contexts/queryOptions';

const RouterTab = createLink(Tab);

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
  // Findings and runs name their rule revision from the definitions.
  const rules = useQuery(ruleDefinitionsOptions());
  const revisions = ruleRevisionsByKey(rules.data ?? []);
  const findingsShown = section === RULE_SECTION_FINDINGS;
  const findings = useInfiniteQuery({
    queryKey: ruleQueryKeys.findingsPage(findingsSearch),
    queryFn: ({ pageParam }) => listFindingsPage(findingsSearch, pageParam),
    initialPageParam: 0,
    getNextPageParam: (page) => page.nextOffset,
    enabled: findingsShown,
  });
  const findingItems = findings.data?.pages.flatMap((page) => page.items);
  // The findings filters offer the workspace's blueprints and contexts.
  const blueprints = useQuery({
    ...blueprintCatalogueOptions(),
    enabled: findingsShown,
  });
  const contexts = useQuery({
    ...contextListOptions(),
    enabled: findingsShown,
  });
  const runs = useQuery({
    queryKey: ruleQueryKeys.runs(),
    queryFn: listRuleRuns,
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
  const acknowledge = useMutation({
    mutationFn: acknowledgeFinding,
    onSuccess: refresh,
  });
  const run = useMutation({
    mutationFn: ({ rule, dryRun }: { rule: Rule; dryRun: boolean }) =>
      runRuleNow(rule.id, rule.version, dryRun),
    onSuccess: refresh,
  });
  const toggle = useMutation({
    mutationFn: ({
      rule,
      enabled,
      acceptExistingViolations,
    }: {
      rule: Rule;
      enabled: boolean;
      acceptExistingViolations?: boolean;
    }) =>
      enabled
        ? enableRuleRevision(rule.id, rule.version, acceptExistingViolations)
        : disableRule(rule.id),
    onSuccess: refresh,
  });
  // Existing violations of an enforcing rule may be accepted explicitly.
  const violationsToAccept =
    toggle.error instanceof ApiRequestError &&
    toggle.error.code === RULE_HAS_EXISTING_VIOLATIONS &&
    toggle.variables
      ? toggle.variables
      : undefined;

  const activeSectionIndex = RULE_SECTIONS.findIndex(
    (item) => item.key === section,
  );

  return (
    <PageContainer>
      <PageHeader
        icon={RuleIcon}
        title={t('rules.title')}
        description={t('rules.description')}
      />
      <Tabs
        aria-label={t('rules.inspectionTabs')}
        sx={{ mt: 3 }}
        value={activeSectionIndex}
      >
        {RULE_SECTIONS.map((item, index) => (
          <RouterTab
            aria-controls={`${RULE_TABPANEL_ID_PREFIX}-${index}`}
            id={`${RULE_TAB_ID_PREFIX}-${index}`}
            key={item.to}
            label={t(`rules.sections.${item.key}`)}
            to={item.to}
            value={index}
          />
        ))}
      </Tabs>
      <Box
        aria-labelledby={`${RULE_TAB_ID_PREFIX}-${activeSectionIndex}`}
        id={`${RULE_TABPANEL_ID_PREFIX}-${activeSectionIndex}`}
        role="tabpanel"
      >
        {section === RULE_SECTION_RULES && run.error && (
          <ApiErrorAlert error={run.error} sx={{ mt: 3 }} />
        )}
        {section === RULE_SECTION_RULES &&
          toggle.error &&
          (violationsToAccept ? (
            <Alert
              action={
                <Button
                  color="inherit"
                  disabled={toggle.isPending}
                  onClick={() =>
                    toggle.mutate({
                      ...violationsToAccept,
                      acceptExistingViolations: true,
                    })
                  }
                  size="small"
                >
                  {t('rules.enableAnyway')}
                </Button>
              }
              severity="warning"
              sx={{ mt: 3 }}
            >
              {errorMessage(toggle.error)}
            </Alert>
          ) : (
            <ApiErrorAlert error={toggle.error} sx={{ mt: 3 }} />
          ))}
        {section === RULE_SECTION_FINDINGS && acknowledge.error && (
          <ApiErrorAlert error={acknowledge.error} sx={{ mt: 3 }} />
        )}
        {section === RULE_SECTION_RULES && (
          <RulesSection
            canManage={canManage}
            error={rules.isError}
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
            error={findings.isError && !findings.isFetchNextPageError}
            recordLabels={recordLabels}
            findings={findingItems}
            filters={findingsSearch}
            onFiltersChange={(search) =>
              void navigate({ to: '/manage/rules/findings', search })
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
            revisions={revisions}
          />
        )}
        {section === RULE_SECTION_RUNS && (
          <RunsSection
            recordLabels={recordLabels}
            error={runs.isError}
            revisions={revisions}
            runs={runs.data}
          />
        )}
      </Box>
    </PageContainer>
  );
};
