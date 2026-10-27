import { blueprintRevisionOptions } from '../blueprints/queryOptions';
import { useQuery } from '@tanstack/react-query';
import { Link } from '@tanstack/react-router';
import { Alert, Box, Button, Paper, Stack, Typography } from '@mui/material';
import { ArrowLeftIcon } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { useResourcePageTitle } from '../../app/useResourcePageTitle';
import { PageContainer } from '../../components/PageContainer';
import { PageHeader } from '../../components/PageHeader';
import { QueryErrorNotice } from '../../components/QueryErrorNotice';
import { RuleIcon } from '../../components/systemIcons';
import { lexiconText } from '../lexicon/lexicon';
import { SHORT_RECORD_ID_LENGTH } from './constants';
import { ruleDefinitionsOptions } from './queryOptions';
import { RuleLifecycleChip } from './RuleCells';

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
  const rules = useQuery(ruleDefinitionsOptions());
  const rule = rules.data?.find(
    (candidate) => candidate.id === ruleId && candidate.version === version,
  );
  const blueprint = useQuery(
    blueprintRevisionOptions(rule?.blueprint_id, rule?.blueprint_version),
  );
  useResourcePageTitle(rule?.name, t('navigation.rules'));

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
      />
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
        <Typography component="h2" id="rule-definition" variant="h6">
          {t('rules.revisionPage.definition')}
        </Typography>
        <Box
          aria-labelledby="rule-definition"
          component="pre"
          sx={{
            fontFamily: 'monospace',
            m: 0,
            mt: 1,
            overflow: 'auto',
            whiteSpace: 'pre-wrap',
          }}
        >
          {rule.definition}
        </Box>
      </Paper>
    </PageContainer>
  );
};
