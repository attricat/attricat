import { Box, Typography } from '@mui/material';
import { useQuery } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import { getSystemHealth } from '../system-health/api';
import { systemHealthQueryKeys } from '../system-health/queryKeys';
import { apiHealthStatusKeys, type ApiHealthState } from './apiHealth';
import {
  API_HEALTH_POLL_INTERVAL_SECONDS,
  shortCommitLength,
} from './constants';

const InspectorBuild = ({ apiHealth }: { apiHealth: ApiHealthState }) => {
  const { t } = useTranslation();
  // Re-enabling after a restart refetches, so a rebuilt API reports its new
  // commit without reloading the page.
  const health = useQuery({
    queryKey: systemHealthQueryKeys.health(),
    queryFn: getSystemHealth,
    enabled: apiHealth === 'ready',
    retry: false,
  });
  if (health.isError)
    return (
      <Typography color="text.secondary" variant="caption">
        {t('inspector.buildUnavailable')}
      </Typography>
    );
  if (!health.data) return null;
  const { branch, commit, version } = health.data.build;
  return (
    <Typography color="text.secondary" title={commit} variant="caption">
      {t('inspector.build', {
        branch,
        commit: commit.slice(0, shortCommitLength),
        version,
      })}
    </Typography>
  );
};

export const InspectorServerPane = ({
  apiHealth,
}: {
  apiHealth: ApiHealthState;
}) => {
  const { t } = useTranslation();
  return (
    <Box sx={{ display: 'grid', gap: 0.5 }}>
      <Typography variant="body2">
        {t('inspector.apiStatus', {
          status: t(apiHealthStatusKeys[apiHealth]),
        })}
      </Typography>
      <InspectorBuild apiHealth={apiHealth} />
      <Typography color="text.secondary" variant="caption">
        {apiHealth === 'restarting'
          ? t('inspector.waitingForHealthCheck')
          : t('inspector.healthCheckInterval', {
              seconds: API_HEALTH_POLL_INTERVAL_SECONDS,
            })}
      </Typography>
    </Box>
  );
};
