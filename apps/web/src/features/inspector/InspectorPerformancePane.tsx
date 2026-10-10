import { Box, Typography } from '@mui/material';
import type { TFunction } from 'i18next';
import { useTranslation } from 'react-i18next';
import { timingDurationFractionDigits, unknownQueryCount } from './constants';
import {
  sqlPhaseName,
  sqlPhasePrefix,
  useTimingStore,
  type TimingPhase,
} from './timing';

const phaseSeparator = ' · ';

const formatPhase = (t: TFunction, phase: TimingPhase) => {
  const duration = phase.duration.toFixed(timingDurationFractionDigits);
  if (!phase.name.startsWith(sqlPhaseName))
    return t('inspector.phaseDuration', { name: phase.name, duration });
  const name =
    phase.name === sqlPhaseName
      ? t('inspector.sqlPhase')
      : t('inspector.sqlPhaseNamed', {
          name: phase.name.slice(sqlPhasePrefix.length).replaceAll('-', ' '),
        });
  return t('inspector.sqlPhaseDuration', {
    name,
    duration,
    queries: t('inspector.queryCount', {
      count: phase.queryCount ?? unknownQueryCount,
    }),
  });
};

export const InspectorPerformancePane = () => {
  const { t } = useTranslation();
  const timings = useTimingStore((state) => state.entries);
  return (
    <Box sx={{ display: 'grid', gap: 0.5 }}>
      {timings.length === 0 ? (
        <Typography color="text.secondary" variant="body2">
          {t('inspector.noTimings')}
        </Typography>
      ) : (
        timings.map((entry) => (
          <Typography key={entry.id} variant="body2">
            {entry.phases
              .map((phase) => formatPhase(t, phase))
              .join(phaseSeparator)}
          </Typography>
        ))
      )}
      <Typography color="text.secondary" variant="caption">
        {t('inspector.timingPrivacy')}
      </Typography>
    </Box>
  );
};
