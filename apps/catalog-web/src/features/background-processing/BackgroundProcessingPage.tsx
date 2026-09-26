import { useQuery } from '@tanstack/react-query';
import {
  Alert,
  Button,
  LinearProgress,
  Paper,
  Stack,
  Table,
  TableBody,
  TableCell,
  TableContainer,
  TableHead,
  TableRow,
  Typography,
} from '@mui/material';
import { useTranslation } from 'react-i18next';
import { PageContainer } from '../../components/PageContainer';
import { PageHeader } from '../../components/PageHeader';
import { currentSession } from '../auth/api';
import { authQueryKeys } from '../auth/queryKeys';
import { getBackgroundProcessingStatus } from './api';
import {
  backgroundProcessingRefreshInterval,
  taskKindTranslationKeys,
} from './constants';
import { backgroundProcessingQueryKeys } from './queryKeys';

export const BackgroundProcessingPage = () => {
  const { t, i18n } = useTranslation();
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
  const number = new Intl.NumberFormat(i18n.language, {
    maximumFractionDigits: 0,
  });

  return (
    <PageContainer>
      <Stack spacing={2}>
        <PageHeader
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
                {t('backgroundProcessing.updated', {
                  time: new Date(status.dataUpdatedAt).toLocaleTimeString(
                    i18n.language,
                  ),
                })}
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
            {!!rows?.length && (
              <TableContainer component={Paper}>
                <Table aria-label={t('backgroundProcessing.title')}>
                  <TableHead>
                    <TableRow>
                      <TableCell>{t('backgroundProcessing.kind')}</TableCell>
                      {[
                        'queued',
                        'running',
                        'failed',
                        'expired',
                        'oldestDue',
                      ].map((column) => (
                        <TableCell key={column} align="right">
                          {t(`backgroundProcessing.${column}`)}
                        </TableCell>
                      ))}
                    </TableRow>
                  </TableHead>
                  <TableBody>
                    {rows.map((row) => (
                      <TableRow key={row.kind}>
                        <TableCell component="th" scope="row">
                          {t(
                            taskKindTranslationKeys[row.kind] ??
                              'backgroundProcessing.kinds.other',
                          )}
                        </TableCell>
                        <TableCell align="right">
                          {number.format(row.queued)}
                        </TableCell>
                        <TableCell align="right">
                          {number.format(row.running)}
                        </TableCell>
                        <TableCell align="right">
                          {number.format(row.failed)}
                        </TableCell>
                        <TableCell align="right">
                          {number.format(row.expired_leases)}
                        </TableCell>
                        <TableCell align="right">
                          {row.oldest_due_seconds === null
                            ? '—'
                            : t('backgroundProcessing.seconds', {
                                value: number.format(row.oldest_due_seconds),
                              })}
                        </TableCell>
                      </TableRow>
                    ))}
                  </TableBody>
                </Table>
              </TableContainer>
            )}
            <Typography variant="body2" color="text.secondary">
              {t('backgroundProcessing.explanation')}
            </Typography>
          </>
        )}
      </Stack>
    </PageContainer>
  );
};
