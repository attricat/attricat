import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { createLink } from '@tanstack/react-router';
import { Alert, Box, Button, Tab, Tabs, Typography } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { ApiRequestError } from '../../api/request';
import { ApiErrorAlert } from '../../components/CheckViolationsAlert';
import { PageContainer } from '../../components/PageContainer';
import { PageHeader } from '../../components/PageHeader';
import { currentSession } from '../auth/api';
import { authQueryKeys } from '../auth/queryKeys';
import {
  acknowledgeFinding,
  disableRule,
  enableRuleRevision,
  listFindings,
  listRuleRuns,
  runRuleNow,
  type Rule,
} from './api';
import {
  FINDING_STATE_RESOLVED,
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
import { useEntityLabels } from '../entities/useEntityLabels';
import { RulesSection } from './RulesSection';
import { RunsSection } from './RunsSection';
import { RuleIcon } from '../../components/systemIcons';

const RouterTab = createLink(Tab);

export type { RuleInspectionSection } from './constants';

export const RuleInspectionPage = ({
  section,
}: {
  section: RuleInspectionSection;
}) => {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const session = useQuery({
    queryKey: authQueryKeys.session(),
    queryFn: currentSession,
  });
  const canRead = session.data?.capabilities?.rules_read === true;
  const canManage = session.data?.capabilities?.rules_manage === true;
  // Findings and runs name their rule revision from the definitions.
  const rules = useQuery({ ...ruleDefinitionsOptions(), enabled: canRead });
  const revisions = ruleRevisionsByKey(rules.data ?? []);
  const findings = useQuery({
    queryKey: ruleQueryKeys.findings(),
    queryFn: () => listFindings(),
    enabled: canRead && section === RULE_SECTION_FINDINGS,
  });
  const runs = useQuery({
    queryKey: ruleQueryKeys.runs(),
    queryFn: listRuleRuns,
    enabled: canRead && section === RULE_SECTION_RUNS,
  });
  // Name the entities the visible findings and runs refer to.
  const entityLabels = useEntityLabels(
    section === RULE_SECTION_FINDINGS
      ? (findings.data ?? [])
          .filter((finding) => finding.state !== FINDING_STATE_RESOLVED)
          .map((finding) => finding.entity_id)
      : section === RULE_SECTION_RUNS
        ? (runs.data ?? []).flatMap((run) =>
            run.scope_entity_id ? [run.scope_entity_id] : [],
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

  if (session.isPending) {
    return (
      <PageContainer>
        <Typography>{t('rules.loading')}</Typography>
      </PageContainer>
    );
  }

  if (!canRead) {
    return (
      <PageContainer>
        <Alert severity="error">{t('rules.notAuthorized')}</Alert>
      </PageContainer>
    );
  }

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
              {toggle.error.message}
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
            error={findings.isError}
            entityLabels={entityLabels}
            findings={findings.data}
            revisions={revisions}
          />
        )}
        {section === RULE_SECTION_RUNS && (
          <RunsSection
            entityLabels={entityLabels}
            error={runs.isError}
            revisions={revisions}
            runs={runs.data}
          />
        )}
      </Box>
    </PageContainer>
  );
};
