import { useQuery } from '@tanstack/react-query';
import { createLink, Link } from '@tanstack/react-router';
import {
  Alert,
  Box,
  Button,
  Paper,
  Stack,
  Tab,
  Tabs,
  Typography,
} from '@mui/material';
import {
  ArrowLeftIcon,
  FileCodeIcon,
  FlaskConicalIcon,
  HistoryIcon,
  PencilIcon,
  PlayIcon,
  PowerIcon,
  PowerOffIcon,
  type LucideIcon,
} from 'lucide-react';
import { Trans, useTranslation } from 'react-i18next';
import { useResourcePageTitle } from '../../app/useResourcePageTitle';
import { compactIconSize } from '../../components/iconSizes';
import { LoadingState } from '../../components/LoadingState';
import { PageContainer } from '../../components/PageContainer';
import { PageHeader } from '../../components/PageHeader';
import { QueryErrorNotice } from '../../components/QueryErrorNotice';
import { RouterButton } from '../../components/RouterLink';
import { RuleIcon } from '../../components/systemIcons';
import { useTabAccessibility } from '../../components/useTabAccessibility';
import { Timestamp } from '../../time/Timestamp';
import { capabilities } from '../auth/capabilities';
import { useCan } from '../auth/useCan';
import { blueprintCatalogueOptions } from '../blueprints/queryOptions';
import { contextLabel } from '../contexts/defaultContext';
import { contextListOptions } from '../contexts/queryOptions';
import { definitionKinds } from '../definition-editor/constants';
import { DefinitionEditor } from '../definition-editor/DefinitionEditor';
import { lexiconText } from '../lexicon/lexicon';
import type { RuleDetails } from './api';
import {
  RULE_DEFINITION_VIEWER_HEIGHT,
  RULE_PAGE_CHANGELOG,
  RULE_PAGE_DEFINITION,
  RULE_PAGE_SECTIONS,
  ruleRoutes,
  type RulePageSection,
} from './constants';
import { recentRuleRunsOptions, ruleOptions } from './queryOptions';
import { RuleChangelog } from './RuleChangelog';
import { RuleFindingCounts, RuleLifecycleChip } from './RuleCells';
import { DryRunDialog } from './DryRunDialog';
import { RunsSection } from './RunsSection';
import { rulesById } from './rulesById';
import { useRunRule } from './useRunRule';
import { useRecordLabels } from '../records/useRecordLabels';
import { ApiErrorAlert } from '../../components/ApiErrorAlert';
import { RuleToggleError } from './RuleToggleError';
import type { RuleComparisonSearch } from './schemas';
import { useToggleRule } from './useToggleRule';
import { FindingSeverityChip } from './FindingSeverityChip';
import { ruleSeverity } from './ruleSeverity';

const RouterTab = createLink(Tab);
const sectionIcons = {
  [RULE_PAGE_DEFINITION]: FileCodeIcon,
  [RULE_PAGE_CHANGELOG]: HistoryIcon,
} satisfies Record<RulePageSection, LucideIcon>;
const noComparison: RuleComparisonSearch = {};
const ignoreChange = () => undefined;

/** A rule's most recent runs, with a link to the full run history. */
const RecentRuns = ({ rule }: { rule: RuleDetails }) => {
  const { t } = useTranslation();
  const runs = useQuery(recentRuleRunsOptions(rule.id));
  const recordLabels = useRecordLabels(
    (runs.data ?? []).flatMap((run) =>
      run.scope_record_id ? [run.scope_record_id] : [],
    ),
  );
  return (
    <RunsSection
      action={
        <Link search={{ rule: rule.id }} to={ruleRoutes.runs}>
          {t('rules.rulePage.allRuns')}
        </Link>
      }
      error={runs.error}
      onRetry={() => void runs.refetch()}
      recordLabels={recordLabels}
      rulesById={rulesById([rule])}
      runs={runs.data}
      showRule={false}
      title={t('rules.rulePage.recentRuns')}
    />
  );
};

/** What a rule checks: its blueprint, context and current definition. */
const RuleDefinition = ({ rule }: { rule: RuleDetails }) => {
  const { t } = useTranslation();
  const blueprints = useQuery(blueprintCatalogueOptions());
  const contexts = useQuery(contextListOptions());
  const severity = ruleSeverity(rule);
  const blueprint = blueprints.data?.find(({ id }) => id === rule.blueprint_id);
  const context = contexts.data?.find(({ id }) => id === rule.context_id);
  return (
    <>
      <Stack
        direction="row"
        spacing={2}
        sx={{ alignItems: 'center', flexWrap: 'wrap', mt: 3 }}
      >
        <RuleLifecycleChip rule={rule} />
        {severity && <FindingSeverityChip severity={severity} />}
        <Typography color="text.secondary" variant="body2">
          {t('rules.rulePage.checks')}{' '}
          <Link
            params={{ blueprintId: rule.blueprint_id }}
            to="/manage/blueprints/$blueprintId"
          >
            {blueprint
              ? lexiconText(blueprint.name)
              : t('rules.unknownBlueprint')}
          </Link>{' '}
          ·{' '}
          {rule.context_id === null
            ? t('rules.recordFindings.allContexts')
            : context
              ? contextLabel(t, context.code)
              : t('rules.unknownContext')}
        </Typography>
        <Typography color="text.secondary" variant="body2">
          <Trans
            components={{
              timestamp: <Timestamp relative value={rule.updated_at} />,
            }}
            i18nKey="rules.rulePage.currentRevision"
            values={{ version: rule.version }}
          />
        </Typography>
        <RuleFindingCounts rule={rule} />
      </Stack>
      {rule.not_applicable.length > 0 && (
        <Alert severity="warning" sx={{ mt: 2 }}>
          {t('rules.rulePage.skipsRevisions')}
          <Box component="ul" sx={{ mb: 0, mt: 0.5, pl: 2.5 }}>
            {rule.not_applicable.map((revision) => (
              <li key={revision.blueprint_version}>
                {t('rules.rulePage.skippedRevision', {
                  version: revision.blueprint_version,
                  reason: t(`rules.inapplicability.${revision.reason}`),
                  message: revision.message,
                })}
              </li>
            ))}
          </Box>
        </Alert>
      )}
      <Paper component="section" sx={{ mt: 3, p: 2 }}>
        <Typography component="h2" variant="h6">
          {t('rules.rulePage.definition')}
        </Typography>
        <DefinitionEditor
          ariaLabel={t('rules.rulePage.definition')}
          height={RULE_DEFINITION_VIEWER_HEIGHT}
          kind={definitionKinds.rule}
          marginTop={1}
          onChange={ignoreChange}
          readOnly
          value={rule.definition}
        />
      </Paper>
      <RecentRuns rule={rule} />
    </>
  );
};

/**
 * One rule: its current definition, or its changelog with a comparison of
 * two revisions.
 */
export const RulePage = ({
  comparison = noComparison,
  ruleId,
  section,
}: {
  /** The revisions the changelog compares, from its URL. */
  comparison?: RuleComparisonSearch;
  ruleId: string;
  section: RulePageSection;
}) => {
  const { t } = useTranslation();
  const canManage = useCan(capabilities.rulesManage);
  const rule = useQuery(ruleOptions(ruleId));
  const toggle = useToggleRule();
  const { run, dryRun, closeDryRun } = useRunRule();
  const tabId = useTabAccessibility();
  useResourcePageTitle(rule.data?.name, t('navigation.rules'));

  const back = (
    <RouterButton
      startIcon={<ArrowLeftIcon />}
      sx={{ mb: 2 }}
      to={ruleRoutes.list}
    >
      {t('rules.rulePage.backToRules')}
    </RouterButton>
  );

  if (rule.isPending) {
    return (
      <PageContainer>
        <LoadingState>{t('rules.loading')}</LoadingState>
      </PageContainer>
    );
  }

  if (!rule.data) {
    return (
      <PageContainer>
        {back}
        <QueryErrorNotice
          error={rule.error}
          isRetrying={rule.isFetching}
          onRetry={() => void rule.refetch()}
        />
      </PageContainer>
    );
  }

  const current = rule.data;
  const activeIndex = RULE_PAGE_SECTIONS.findIndex(
    (item) => item.key === section,
  );
  return (
    <PageContainer>
      {back}
      <PageHeader
        actions={
          canManage && (
            <Stack direction="row" spacing={1} sx={{ flexWrap: 'wrap' }}>
              <Button
                disabled={run.isPending}
                onClick={() => run.mutate({ rule: current, dryRun: true })}
                startIcon={<FlaskConicalIcon />}
                variant="outlined"
              >
                {t('rules.dryRun')}
              </Button>
              {current.enabled && (
                <Button
                  disabled={run.isPending}
                  onClick={() => run.mutate({ rule: current, dryRun: false })}
                  startIcon={<PlayIcon />}
                  variant="outlined"
                >
                  {t('rules.runNow')}
                </Button>
              )}
              <RouterButton
                params={{ ruleId }}
                startIcon={<PencilIcon />}
                to={ruleRoutes.edit}
                variant="outlined"
              >
                {t('rules.rulePage.edit')}
              </RouterButton>
              <Button
                disabled={toggle.isPending}
                onClick={() =>
                  toggle.mutate({ rule: current, enabled: !current.enabled })
                }
                startIcon={current.enabled ? <PowerOffIcon /> : <PowerIcon />}
                variant="outlined"
              >
                {current.enabled
                  ? t('rules.rulePage.disable')
                  : t('rules.rulePage.enable')}
              </Button>
            </Stack>
          )
        }
        description={current.code}
        icon={RuleIcon}
        title={current.name}
      />
      <RuleToggleError sx={{ mt: 2 }} toggle={toggle} />
      <ApiErrorAlert error={run.error} sx={{ mt: 2 }} />
      <DryRunDialog dryRun={dryRun} onClose={closeDryRun} />
      <Tabs
        aria-label={t('rules.rulePage.tabs')}
        sx={{ mt: 3 }}
        value={activeIndex}
      >
        {RULE_PAGE_SECTIONS.map((item, index) => {
          const Icon = sectionIcons[item.key];
          return (
            <RouterTab
              {...tabId.tab(index)}
              icon={<Icon aria-hidden size={compactIconSize} />}
              key={item.key}
              label={t(`rules.rulePage.sections.${item.key}`)}
              params={{ ruleId }}
              to={item.to}
              value={index}
            />
          );
        })}
      </Tabs>
      <Box {...tabId.panel(activeIndex)}>
        {section === RULE_PAGE_DEFINITION && <RuleDefinition rule={current} />}
        {section === RULE_PAGE_CHANGELOG && (
          <RuleChangelog comparison={comparison} rule={current} />
        )}
      </Box>
    </PageContainer>
  );
};
