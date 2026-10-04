import {
  Alert,
  Box,
  Button,
  Card,
  List,
  ListItem,
  ListItemText,
  Skeleton,
  Stack,
  Typography,
} from '@mui/material';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { ArrowLeftIcon, DownloadIcon } from 'lucide-react';
import { Trans, useTranslation } from 'react-i18next';
import { RouterButton } from '../../components/RouterLink';
import { Timestamp } from '../../time/Timestamp';
import { formatBytes } from '../data-health/dataHealthFormat';
import { cancelExtensionRun, extensionRunArtifactUrl } from './api';
import { extensionRunsPagePath, runDetailSkeletonHeight } from './constants';
import {
  ExtensionRunProgress,
  ExtensionRunStatusChip,
} from './ExtensionRunSummary';
import { extensionRunQueryKeys } from './queryKeys';
import { extensionRunDetailOptions } from './queryOptions';
import type { ExtensionRunDetail } from './schemas';

const Outputs = ({ run }: { run: ExtensionRunDetail }) => {
  const { t } = useTranslation();
  if (run.status !== 'completed')
    return (
      <Typography color="text.secondary" variant="body2">
        {t('extensionRuns.outputsAfterCompletion')}
      </Typography>
    );
  if (run.outputs_expired)
    return <Alert severity="info">{t('extensionRuns.outputsExpired')}</Alert>;
  if (run.artifacts.length === 0)
    return (
      <Typography color="text.secondary" variant="body2">
        {t('extensionRuns.noOutputs')}
      </Typography>
    );
  return (
    <Stack spacing={2}>
      {run.outputs_expire_at && (
        <Typography color="text.secondary" variant="body2">
          <Trans
            components={{
              timestamp: (
                <Timestamp style="dateTime" value={run.outputs_expire_at} />
              ),
            }}
            i18nKey="extensionRuns.outputsAvailableUntil"
          />
        </Typography>
      )}
      <List dense disablePadding>
        {run.artifacts.map((artifact) => (
          <ListItem
            disableGutters
            key={artifact.id}
            secondaryAction={
              <Button
                component="a"
                download
                href={extensionRunArtifactUrl(run.id, artifact.id)}
                size="small"
                startIcon={<DownloadIcon />}
              >
                {t('extensionRuns.download')}
              </Button>
            }
          >
            <ListItemText
              primary={artifact.name ?? t('extensionRuns.unnamedOutput')}
              secondary={`${artifact.media_type} · ${formatBytes(artifact.content_length)}`}
            />
          </ListItem>
        ))}
      </List>
    </Stack>
  );
};

/** One run's status, outcome, and authorized downloads. */
export const ExtensionRunDetailPage = ({ runId }: { runId: string }) => {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const run = useQuery(extensionRunDetailOptions(runId));
  const cancel = useMutation({
    mutationFn: () => cancelExtensionRun(runId),
    onSettled: () =>
      queryClient.invalidateQueries({ queryKey: extensionRunQueryKeys.all() }),
  });
  return (
    <Stack spacing={4}>
      <Box>
        <RouterButton
          size="small"
          startIcon={<ArrowLeftIcon />}
          to={extensionRunsPagePath}
        >
          {t('extensionRuns.backToRuns')}
        </RouterButton>
      </Box>
      {run.isPending && (
        <Skeleton height={runDetailSkeletonHeight} variant="rounded" />
      )}
      {run.isError && (
        <Alert severity="error">{t('extensionRuns.loadFailed')}</Alert>
      )}
      {run.data && (
        <Card sx={{ display: 'flex', flexDirection: 'column', gap: 4, p: 5 }}>
          <Stack
            direction={{ xs: 'column', sm: 'row' }}
            spacing={3}
            sx={{ alignItems: { sm: 'flex-start' } }}
          >
            <Box sx={{ flex: 1, minWidth: 0 }}>
              <Typography
                component="h2"
                sx={{ overflowWrap: 'anywhere' }}
                variant="h6"
              >
                {run.data.operation_id}
              </Typography>
              <Typography color="text.secondary" variant="body2">
                {run.data.extension_id}
              </Typography>
            </Box>
            <ExtensionRunStatusChip status={run.data.status} />
          </Stack>
          <Typography color="text.secondary" variant="body2">
            {t('extensionRuns.entityCount', {
              count: run.data.selection_count,
            })}
            {' · '}
            <Timestamp style="dateTime" value={run.data.created_at} />
          </Typography>
          {run.data.failure && (
            <Alert severity="error">
              {t(`extensionRuns.failure.${run.data.failure}`)}
            </Alert>
          )}
          <ExtensionRunProgress run={run.data} />
          <Box component="section">
            <Typography component="h3" gutterBottom variant="subtitle1">
              {t('extensionRuns.outputs')}
            </Typography>
            <Outputs run={run.data} />
          </Box>
          {cancel.isError && (
            <Alert severity="error">{t('extensionRuns.cancelFailed')}</Alert>
          )}
          {run.data.can_cancel && (
            <Box>
              <Button
                color="error"
                disabled={cancel.isPending}
                onClick={() => cancel.mutate()}
                variant="outlined"
              >
                {t('extensionRuns.cancel')}
              </Button>
            </Box>
          )}
        </Card>
      )}
    </Stack>
  );
};
