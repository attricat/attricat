import { Box, Typography } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { apiHealthStatusKeys, type ApiHealthState } from './apiHealth';
import { API_HEALTH_POLL_INTERVAL_SECONDS } from './constants';

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
