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
  HistoryIcon,
  PencilIcon,
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
import { ruleOptions } from './queryOptions';
import { RuleChangelog } from './RuleChangelog';
import { RuleLifecycleChip } from './RuleCells';
import { RuleToggleError } from './RuleToggleError';
import type { RuleComparisonSearch } from './schemas';
import { useToggleRule } from './useToggleRule';

const RouterTab = createLink(Tab);
const sectionIcons = {
  [RULE_PAGE_DEFINITION]: FileCodeIcon,
  [RULE_PAGE_CHANGELOG]: HistoryIcon,
} satisfies Record<RulePageSection, LucideIcon>;
const noComparison: RuleComparisonSearch = {};
const ignoreChange = () => undefined;

/** What a rule checks: its blueprint, context and current definition. */
const RuleDefinition = ({ rule }: { rule: RuleDetails }) => {
  const { t } = useTranslation();
  const blueprints = useQuery(blueprintCatalogueOptions());
  const contexts = useQuery(contextListOptions());
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
