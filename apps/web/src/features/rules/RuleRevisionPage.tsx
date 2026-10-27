import { blueprintRevisionOptions } from '../blueprints/queryOptions';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Link } from '@tanstack/react-router';
import { Alert, Button, Paper, Stack, Typography } from '@mui/material';
import { ArrowLeftIcon, PencilIcon, SendIcon } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { useResourcePageTitle } from '../../app/useResourcePageTitle';
import { revisionStatuses } from '../../api/revisionStatus';
import { ApiErrorAlert } from '../../components/ApiErrorAlert';
import { PageContainer } from '../../components/PageContainer';
import { PageHeader } from '../../components/PageHeader';
import { QueryErrorNotice } from '../../components/QueryErrorNotice';
import { RouterButton } from '../../components/RouterLink';
import { RuleIcon } from '../../components/systemIcons';
import { useCan } from '../auth/useCan';
import { capabilities } from '../auth/capabilities';
import { definitionKinds } from '../definition-editor/constants';
import { DefinitionEditor } from '../definition-editor/DefinitionEditor';
import { lexiconText } from '../lexicon/lexicon';
import { publishRuleRevision } from './api';
import {
  RULE_DEFINITION_VIEWER_HEIGHT,
  ruleRoutes,
  SHORT_RECORD_ID_LENGTH,
} from './constants';
import { ruleQueryKeys } from './queryKeys';
import { ruleDefinitionsOptions } from './queryOptions';
import { RuleLifecycleChip } from './RuleCells';

const ignoreChange = () => undefined;

/** One rule revision with the definition its findings were checked against. */
export const RuleRevisionPage = ({
  ruleId,
  version,
}: {
  ruleId: string;
  /** Undefined when the URL names no valid version. */
  version: number | undefined;
}) => {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const canManage = useCan(capabilities.rulesManage);
  const rules = useQuery(ruleDefinitionsOptions());
  const rule = rules.data?.find(
    (candidate) => candidate.id === ruleId && candidate.version === version,
  );
  const blueprint = useQuery(
    blueprintRevisionOptions(rule?.blueprint_id, rule?.blueprint_version),
  );
  useResourcePageTitle(rule?.name, t('navigation.rules'));
  const publish = useMutation({
    mutationFn: (revision: number) => publishRuleRevision(ruleId, revision),
    onSuccess: () =>
      queryClient.invalidateQueries({ queryKey: ruleQueryKeys.definitions() }),
  });

  const back = (
    <Button
      component={Link}
      to="/manage/rules"
      startIcon={<ArrowLeftIcon />}
      sx={{ mb: 2 }}
    >
      {t('rules.revisionPage.backToRules')}
    </Button>
  );

  if (rules.isPending) {
    return (
      <PageContainer>
        <Typography>{t('rules.loading')}</Typography>
      </PageContainer>
    );
  }

  if (!rule) {
    return (
      <PageContainer>
        {back}
        <QueryErrorNotice
          error={rules.error}
          isRetrying={rules.isFetching}
          onRetry={() => void rules.refetch()}
        />
        {rules.isSuccess && (
          <Alert severity="warning">{t('rules.revisionPage.notFound')}</Alert>
        )}
      </PageContainer>
    );
  }

  return (
    <PageContainer>
      {back}
      <PageHeader
        icon={RuleIcon}
        title={t('rules.ruleRevision', {
          name: rule.name,
          version: rule.version,
        })}
        description={rule.code}
        actions={
          canManage && (
            <Stack direction="row" spacing={1} sx={{ flexWrap: 'wrap' }}>
              <RouterButton
                params={{ ruleId: rule.id, version: String(rule.version) }}
                startIcon={<PencilIcon />}
                to={ruleRoutes.newRevision}
                variant="outlined"
              >
                {t('rules.newRevision')}
              </RouterButton>
              {rule.status === revisionStatuses.draft && (
                <Button
                  disabled={publish.isPending}
                  onClick={() => publish.mutate(rule.version)}
                  startIcon={<SendIcon />}
                  variant="contained"
                >
                  {t('rules.publish')}
                </Button>
              )}
            </Stack>
          )
        }
      />
      <ApiErrorAlert error={publish.error} sx={{ mt: 2 }} />
      <Stack
        direction="row"
        spacing={2}
        sx={{ alignItems: 'center', flexWrap: 'wrap', mt: 2 }}
      >
        <RuleLifecycleChip rule={rule} />
        <Typography color="text.secondary" variant="body2">
          {t('rules.revisionPage.blueprint')}{' '}
          <Link
            params={{ blueprintId: rule.blueprint_id }}
            to="/manage/blueprints/$blueprintId"
          >
            {t('rules.revisionPage.blueprintRevision', {
              name: blueprint.data
                ? lexiconText(blueprint.data.blueprint.name)
                : rule.blueprint_id.slice(0, SHORT_RECORD_ID_LENGTH),
              version: rule.blueprint_version,
            })}
          </Link>
        </Typography>
      </Stack>
      <Paper component="section" sx={{ mt: 3, p: 2 }}>
        <Typography component="h2" variant="h6">
          {t('rules.revisionPage.definition')}
        </Typography>
        <DefinitionEditor
          ariaLabel={t('rules.revisionPage.definition')}
          height={RULE_DEFINITION_VIEWER_HEIGHT}
          kind={definitionKinds.rule}
          marginTop={1}
          onChange={ignoreChange}
          readOnly
          value={rule.definition}
        />
      </Paper>
    </PageContainer>
  );
};
