import { Box, Chip, LinearProgress, Stack, Typography } from '@mui/material';
import {
  BanIcon,
  CircleCheckIcon,
  CircleXIcon,
  ClockIcon,
  LoaderIcon,
} from 'lucide-react';
import type { ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { Timestamp } from '../../time/Timestamp';
import type { ExtensionRunStatus } from './constants';
import { isActiveExtensionRun } from './runPolling';
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
 * The operation, extension and status of a run, then when it started and on
 * how many entities. `heading` is the operation title in its page outline.
 */
export const ExtensionRunHeader = ({
  heading,
  icon,
  run,
}: {
  heading: { component: 'h2' | 'h3'; variant: 'h6' | 'subtitle1' };
  icon?: ReactNode;
  run: ExtensionRun;
}) => {
  const { t } = useTranslation();
  return (
    <>
      {/* A leading icon keeps the row layout on narrow screens. */}
      <Stack
        direction={icon ? 'row' : { xs: 'column', sm: 'row' }}
        spacing={3}
        sx={{ alignItems: icon ? 'flex-start' : { sm: 'flex-start' } }}
      >
        {icon}
        <Box sx={{ flex: 1, minWidth: 0 }}>
          <Typography
            component={heading.component}
            sx={{ overflowWrap: 'anywhere' }}
            variant={heading.variant}
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
    </>
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
  const active = isActiveExtensionRun(run);
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
