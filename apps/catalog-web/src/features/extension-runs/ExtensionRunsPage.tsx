import { Alert, Box, Card, Skeleton, Stack, Typography } from '@mui/material';
import { useQuery } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import { RouterButton } from '../../components/RouterLink';
import { smallIconSize } from '../../components/iconSizes';
import { ExtensionRunIcon } from '../../components/systemIcons';
import { Timestamp } from '../../time/Timestamp';
import { listExtensionRuns } from './api';
import {
  activeExtensionRunStatuses,
  extensionRunPollMilliseconds,
  runSkeletonCount,
  runSkeletonHeight,
} from './constants';
import {
  ExtensionRunProgress,
  ExtensionRunStatusChip,
} from './ExtensionRunSummary';
import { extensionRunQueryKeys } from './queryKeys';
import type { ExtensionRun } from './schemas';

const RunCard = ({ run }: { run: ExtensionRun }) => {
  const { t } = useTranslation();
  return (
    <Card
      component="li"
      sx={{ display: 'flex', flexDirection: 'column', gap: 3, p: 5 }}
    >
      <Stack direction="row" spacing={3} sx={{ alignItems: 'flex-start' }}>
        <Box sx={{ color: 'text.secondary', display: 'flex', pt: 0.25 }}>
          <ExtensionRunIcon aria-hidden size={smallIconSize} />
        </Box>
        <Box sx={{ flex: 1, minWidth: 0 }}>
          <Typography
            component="h3"
            sx={{ overflowWrap: 'anywhere' }}
            variant="subtitle1"
          >
            {run.operation_id}
          </Typography>
          <Typography color="text.secondary" variant="body2">
            {run.extension_id}
          </Typography>
        </Box>
        <ExtensionRunStatusChip status={run.status} />
      </Stack>
      <Typography color="text.secondary" variant="body2">
        {t('extensionRuns.entityCount', { count: run.selection_count })}
        {' · '}
        <Timestamp style="dateTime" value={run.created_at} />
      </Typography>
      <ExtensionRunProgress run={run} />
      <Box>
        <RouterButton
          params={{ runId: run.id }}
          size="small"
          to="/profile/extension-runs/$runId"
          variant="outlined"
        >
          {t('extensionRuns.viewRun')}
        </RouterButton>
      </Box>
    </Card>
  );
};

/** The signed-in user's recent extension runs, newest first. */
export const ExtensionRunsPage = () => {
  const { t } = useTranslation();
  const runs = useQuery({
    queryKey: extensionRunQueryKeys.list(),
    queryFn: () => listExtensionRuns(),
    refetchInterval: (query) =>
      query.state.data?.some((run) =>
        activeExtensionRunStatuses.includes(run.status),
      )
        ? extensionRunPollMilliseconds
        : false,
  });
  return (
    <Stack spacing={5}>
      <Typography color="text.secondary" variant="body2">
        {t('extensionRuns.description')}
      </Typography>
      {runs.isPending && (
        <Stack spacing={4}>
          {Array.from({ length: runSkeletonCount }, (_, index) => (
            <Skeleton
              height={runSkeletonHeight}
              key={index}
              variant="rounded"
            />
          ))}
        </Stack>
      )}
      {runs.isError && (
        <Alert severity="error">{t('extensionRuns.loadFailed')}</Alert>
      )}
      {runs.data?.length === 0 && (
        <Typography color="text.secondary">
          {t('extensionRuns.empty')}
        </Typography>
      )}
      {runs.data && runs.data.length > 0 && (
        <Stack
          component="ul"
          spacing={4}
          sx={{ listStyle: 'none', m: 0, p: 0 }}
        >
          {runs.data.map((run) => (
            <RunCard key={run.id} run={run} />
          ))}
        </Stack>
      )}
    </Stack>
  );
};
