import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { createLink } from '@tanstack/react-router';
import { Alert, Box, Tab, Tabs, Typography } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { PageContainer } from '../../components/PageContainer';
import { PageHeader } from '../../components/PageHeader';
import { currentSession } from '../auth/api';
import { authQueryKeys } from '../auth/queryKeys';
import {
  acknowledgeFinding,
  listFindings,
  listRuleRuns,
  listRules,
  runRuleNow,
} from './api';
import {
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
  const rules = useQuery({
    queryKey: ruleQueryKeys.definitions(),
    queryFn: listRules,
    enabled: canRead && section === RULE_SECTION_RULES,
  });
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
  const refresh = () =>
    queryClient.invalidateQueries({ queryKey: ruleQueryKeys.all });
  const acknowledge = useMutation({
    mutationFn: acknowledgeFinding,
    onSuccess: refresh,
  });
  const run = useMutation({
    mutationFn: ({ id, dryRun }: { id: string; dryRun: boolean }) =>
      runRuleNow(id, dryRun),
    onSuccess: refresh,
  });

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
        {section === RULE_SECTION_RULES && (
          <RulesSection
            canManage={canManage}
            error={rules.isError}
            onRun={(id, dryRun) => run.mutate({ id, dryRun })}
            running={run.isPending}
            rules={rules.data}
          />
        )}
        {section === RULE_SECTION_FINDINGS && (
          <FindingsSection
            onAcknowledge={(id) => acknowledge.mutate(id)}
            acknowledging={acknowledge.isPending}
            canManage={canManage}
            error={findings.isError}
            findings={findings.data}
          />
        )}
        {section === RULE_SECTION_RUNS && (
          <RunsSection error={runs.isError} runs={runs.data} />
        )}
      </Box>
    </PageContainer>
  );
};
