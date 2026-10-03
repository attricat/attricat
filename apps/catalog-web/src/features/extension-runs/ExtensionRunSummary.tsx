import { Box, Chip, LinearProgress, Stack, Typography } from '@mui/material';
import {
  BanIcon,
  CircleCheckIcon,
  CircleXIcon,
  ClockIcon,
  LoaderIcon,
} from 'lucide-react';
import { useTranslation } from 'react-i18next';
import type { ExtensionRunStatus } from './constants';
import {
  runOutcomeSchema,
  runProgressCountsSchema,
  type ExtensionRun,
} from './schemas';

const statusChips = {
  queued: { color: 'default', icon: ClockIcon },
  running: { color: 'info', icon: LoaderIcon },
  cancelling: { color: 'warning', icon: LoaderIcon },
  cancelled: { color: 'default', icon: BanIcon },
  completed: { color: 'success', icon: CircleCheckIcon },
  failed: { color: 'error', icon: CircleXIcon },
} as const satisfies Record<ExtensionRunStatus, unknown>;

/** Execution status; never infers the extension's domain outcome. */
export const ExtensionRunStatusChip = ({
  status,
}: {
  status: ExtensionRunStatus;
}) => {
  const { t } = useTranslation();
  const chip = statusChips[status];
  return (
    <Chip
      color={chip.color}
      icon={<chip.icon />}
      label={t(`extensionRuns.status.${status}`)}
      variant="outlined"
    />
  );
};

/**
 * Progress and the extension-reported outcome. A completed run can still
 * report per-entity failures; that is shown separately from its status.
 */
export const ExtensionRunProgress = ({ run }: { run: ExtensionRun }) => {
  const { t } = useTranslation();
  const counts = runProgressCountsSchema.safeParse(run.progress);
  const outcome = runOutcomeSchema.safeParse(run.progress.outcome);
  const active = ['queued', 'running', 'cancelling'].includes(run.status);
  const outcomeParts = outcome.success
    ? (['succeeded', 'failed', 'skipped'] as const).flatMap((key) =>
        outcome.data[key] === undefined
          ? []
          : [t(`extensionRuns.outcome.${key}`, { count: outcome.data[key] })],
      )
    : [];
  return (
    <Stack spacing={1}>
      {counts.success ? (
        <>
          <LinearProgress
            aria-label={t('extensionRuns.progressLabel')}
            value={Math.min(
              100,
              (counts.data.completed / counts.data.total) * 100,
            )}
            variant="determinate"
          />
          <Typography color="text.secondary" variant="body2">
            {t('extensionRuns.progress', {
              completed: counts.data.completed,
              total: counts.data.total,
            })}
          </Typography>
        </>
      ) : (
        active && (
          <LinearProgress aria-label={t('extensionRuns.progressLabel')} />
        )
      )}
      {outcomeParts.length > 0 && (
        <Box>
          <Typography variant="body2">{outcomeParts.join(' · ')}</Typography>
        </Box>
      )}
    </Stack>
  );
};
