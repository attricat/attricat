import { Alert, Box, Card, Skeleton, Stack, Typography } from '@mui/material';
import { useQuery } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import { RouterButton } from '../../components/RouterLink';
import { smallIconSize } from '../../components/iconSizes';
import { EmptyState } from '../../components/EmptyState';
import { ExtensionRunIcon } from '../../components/systemIcons';
import { runSkeletonCount, runSkeletonHeight } from './constants';
import {
  ExtensionRunHeader,
  ExtensionRunProgress,
} from './ExtensionRunSummary';
import { extensionRunListOptions } from './queryOptions';
import type { ExtensionRun } from './schemas';

const RunCard = ({ run }: { run: ExtensionRun }) => {
  const { t } = useTranslation();
  return (
    <Card
      component="li"
      sx={{ display: 'flex', flexDirection: 'column', gap: 3, p: 5 }}
    >
      <ExtensionRunHeader
        heading={{ component: 'h3', variant: 'subtitle1' }}
        icon={
          <Box sx={{ color: 'text.secondary', display: 'flex', pt: 0.25 }}>
            <ExtensionRunIcon aria-hidden size={smallIconSize} />
          </Box>
        }
        run={run}
      />
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
  const runs = useQuery(extensionRunListOptions());
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
        <EmptyState icon={ExtensionRunIcon} title={t('extensionRuns.empty')} />
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
