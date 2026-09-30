import { useQuery } from '@tanstack/react-query';
import {
  Alert,
  Button,
  LinearProgress,
  Stack,
  Typography,
} from '@mui/material';
import { Trans, useTranslation } from 'react-i18next';
import { PageContainer } from '../../components/PageContainer';
import { PageHeader } from '../../components/PageHeader';
import { Timestamp } from '../../time/Timestamp';
import { currentSession } from '../auth/api';
import { authQueryKeys } from '../auth/queryKeys';
import { getBackgroundProcessingStatus } from './api';
import { BackgroundProcessingTable } from './BackgroundProcessingTable';
import {
  backgroundProcessingRefreshInterval,
  backgroundProcessingRefreshSeconds,
} from './constants';
import { backgroundProcessingQueryKeys } from './queryKeys';
import { BackgroundProcessingIcon } from '../../components/systemIcons';

export const BackgroundProcessingPage = () => {
  const { t } = useTranslation();
  const session = useQuery({
    queryKey: authQueryKeys.session(),
    queryFn: currentSession,
  });
  const canRead = session.data?.capabilities?.data_health_read === true;
  const status = useQuery({
    queryKey: backgroundProcessingQueryKeys.status(session.data?.workspace_id),
    queryFn: getBackgroundProcessingStatus,
    enabled: canRead,
    refetchInterval: backgroundProcessingRefreshInterval,
  });
  const rows = canRead ? status.data : undefined;
  const hasFailures = rows?.some((row) => row.failed > 0);
  const hasExpiredLeases = rows?.some((row) => row.expired_leases > 0);

  return (
    <PageContainer>
      <Stack spacing={2}>
        <PageHeader
          icon={BackgroundProcessingIcon}
          title={t('backgroundProcessing.title')}
          description={t('backgroundProcessing.description')}
          actions={
            canRead && (
              <Button
                disabled={status.isFetching}
                onClick={() => void status.refetch()}
              >
                {t(
                  status.isFetching
                    ? 'backgroundProcessing.refreshing'
                    : 'backgroundProcessing.refresh',
                )}
              </Button>
            )
          }
        />
        {session.isPending && (
          <LinearProgress aria-label={t('backgroundProcessing.loading')} />
        )}
        {session.isError && (
          <Alert
            severity="error"
            action={
              <Button color="inherit" onClick={() => void session.refetch()}>
                {t('backgroundProcessing.refresh')}
              </Button>
            }
          >
            {t('backgroundProcessing.loadError')}
          </Alert>
        )}
        {session.isSuccess && !canRead && (
          <Alert severity="warning">
            {t('backgroundProcessing.forbidden')}
          </Alert>
        )}
        {canRead && (
          <>
            {status.isPending && (
              <LinearProgress aria-label={t('backgroundProcessing.loading')} />
            )}
            {status.isError && (
              <Alert severity="error">
                {t(
                  rows
                    ? 'backgroundProcessing.refreshError'
                    : 'backgroundProcessing.loadError',
                )}
              </Alert>
            )}
            {rows && (
              <Typography variant="body2" color="text.secondary">
                <Trans
                  components={{
                    timestamp: (
                      <Timestamp style="time" value={status.dataUpdatedAt} />
                    ),
                  }}
                  i18nKey="backgroundProcessing.updated"
                  t={t}
                  values={{ seconds: backgroundProcessingRefreshSeconds }}
                />
              </Typography>
            )}
            {hasFailures && (
              <Alert severity="error">
                {t('backgroundProcessing.failures')}
              </Alert>
            )}
            {hasExpiredLeases && (
              <Alert severity="warning">
                {t('backgroundProcessing.expiredWarning')}
              </Alert>
            )}
            {rows?.length === 0 && (
              <Alert severity="info">{t('backgroundProcessing.empty')}</Alert>
            )}
            {!!rows?.length && <BackgroundProcessingTable rows={rows} />}
            <Typography variant="body2" color="text.secondary">
              {t('backgroundProcessing.explanation')}
            </Typography>
          </>
        )}
      </Stack>
    </PageContainer>
  );
};
