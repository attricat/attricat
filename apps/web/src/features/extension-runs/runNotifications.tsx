import { Button } from '@mui/material';
import type { UseNavigateResult } from '@tanstack/react-router';
import { toast } from '../../components/toast';
import i18n from '../../i18n';
import type { ExtensionRun } from './schemas';
import { useRunWatchStore } from './runWatchStore';

type Navigate = UseNavigateResult<string>;

// Toasts render outside the router, so actions navigate through a function
// captured by a component that is inside it.
const viewRunAction = (runId: string, navigate: Navigate) => (
  <Button
    color="inherit"
    onClick={() =>
      void navigate({
        to: '/profile/extension-runs/$runId',
        params: { runId },
      })
    }
    size="small"
  >
    {i18n.t('extensionRuns.viewRun')}
  </Button>
);

/** Tracks a newly started run and tells the user where to follow it. */
export const notifyRunStarted = (runId: string, navigate: Navigate) => {
  useRunWatchStore.getState().watch(runId);
  toast.info(i18n.t('extensionRuns.started'), {
    action: viewRunAction(runId, navigate),
  });
};

/** Announces a watched run's terminal execution status. */
export const notifyRunFinished = (run: ExtensionRun, navigate: Navigate) => {
  const action = viewRunAction(run.id, navigate);
  if (run.status === 'completed')
    toast.success(i18n.t('extensionRuns.finished'), { action });
  else if (run.status === 'cancelled')
    toast.info(i18n.t('extensionRuns.cancelledNotice'), { action });
  else toast.error(i18n.t('extensionRuns.failedNotice'), { action });
};
