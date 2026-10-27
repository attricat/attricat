import { useQuery } from '@tanstack/react-query';
import { Box, Card, LinearProgress, Stack, Typography } from '@mui/material';
import { GitBranchIcon } from 'lucide-react';
import { Fragment, useId } from 'react';
import { useTranslation } from 'react-i18next';
import { monoFontFamily } from '../../app/theme';
import { smallIconSize } from '../../components/iconSizes';
import { PageContainer } from '../../components/PageContainer';
import { PageHeader } from '../../components/PageHeader';
import { QueryErrorNotice } from '../../components/QueryErrorNotice';
import { SystemHealthIcon } from '../../components/systemIcons';
import { RequireCapability } from '../auth/RequireCapability';
import type { SystemHealth } from './schemas';
import { capabilities } from '../auth/capabilities';
import { systemHealthOptions } from './queryOptions';

const BuildDetails = ({ build }: { build: SystemHealth['build'] }) => {
  const { t } = useTranslation();
  const headingId = useId();
  const details = [
    ['systemHealth.version', build.version],
    ['systemHealth.branch', build.branch],
    ['systemHealth.commit', build.commit],
  ] as const;

  return (
    <Card aria-labelledby={headingId} component="section" sx={{ p: 6 }}>
      <Stack direction="row" spacing={3} sx={{ mb: 5 }}>
        <Box sx={{ color: 'text.secondary', display: 'flex', pt: 0.25 }}>
          <GitBranchIcon aria-hidden size={smallIconSize} />
        </Box>
        <Box>
          <Typography component="h2" id={headingId} variant="h4">
            {t('systemHealth.buildTitle')}
          </Typography>
          <Typography color="text.secondary" sx={{ mt: 1 }} variant="body2">
            {t('systemHealth.buildDescription')}
          </Typography>
        </Box>
      </Stack>
      <Box
        component="dl"
        sx={{
          color: 'text.secondary',
          columnGap: 4,
          display: 'grid',
          gridTemplateColumns: 'max-content 1fr',
          m: 0,
          rowGap: 2,
          typography: 'body2',
          '& dd': {
            color: 'text.primary',
            fontFamily: monoFontFamily,
            m: 0,
            minWidth: 0,
            overflowWrap: 'anywhere',
          },
        }}
      >
        {details.map(([label, value]) => (
          <Fragment key={label}>
            <dt>{t(label)}</dt>
            <dd>{value}</dd>
          </Fragment>
        ))}
      </Box>
    </Card>
  );
};

const SystemHealthDetails = () => {
  const { t } = useTranslation();
  const health = useQuery(systemHealthOptions());

  return (
    <PageContainer>
      <Stack spacing={4}>
        <PageHeader
          icon={SystemHealthIcon}
          title={t('systemHealth.title')}
          description={t('systemHealth.description')}
        />
        {health.isPending && (
          <LinearProgress aria-label={t('systemHealth.loading')} />
        )}
        <QueryErrorNotice
          error={health.error}
          isRetrying={health.isFetching}
          onRetry={() => void health.refetch()}
        />
        {health.data && <BuildDetails build={health.data.build} />}
      </Stack>
    </PageContainer>
  );
};

export const SystemHealthPage = () => {
  const { t } = useTranslation();
  return (
    <RequireCapability
      capability={capabilities.dataHealthRead}
      deniedMessage={t('systemHealth.forbidden')}
    >
      <SystemHealthDetails />
    </RequireCapability>
  );
};
