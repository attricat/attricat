import { Box, Button, Typography } from '@mui/material';
import type { TFunction } from 'i18next';
import { useTranslation } from 'react-i18next';
import {
  brokerOutcomes,
  extensionRunStates,
  useExtensionDiagnosticsStore,
  type BrokerRequestRecord,
  type ExtensionRunRecord,
} from '../extensions/diagnostics/extensionDiagnosticsStore';
import { renderingModes } from '../extensions/constants';
import { nativeUiBudgets } from '../extensions/native/constants';
import { timingDurationFractionDigits } from './constants';
import type { InstantStyle } from '../../time/instantFormat';
import { useInstantFormat } from '../../time/useInstantFormat';

/** How many recent requests and drops the pane lists. */
const shownRequests = 20;
const shownDrops = 10;
/** Requests are recent, so their time of day says enough. */
const requestTimeStyle: InstantStyle = 'time';
const bytesPerKibibyte = 1_024;
const detailSeparator = ' · ';

const milliseconds = (value: number) =>
  value.toFixed(timingDurationFractionDigits);

const runLine = (t: TFunction, run: ExtensionRunRecord) => {
  const details = [
    run.rendering === renderingModes.native
      ? t('inspector.extensions.native', { uiKit: run.uiKit ?? '?' })
      : t('inspector.extensions.frame', { contribution: run.contributionId }),
    run.devRevision ? t('inspector.extensions.devBuild') : undefined,
    run.state === extensionRunStates.ready
      ? t('inspector.extensions.readyIn', {
          ms: milliseconds(run.startMs ?? 0),
        })
      : run.state === extensionRunStates.failed
        ? t('inspector.extensions.failed', { code: run.errorCode })
        : t('inspector.extensions.starting'),
    run.heartbeatMs === undefined
      ? undefined
      : t('inspector.extensions.heartbeat', {
          ms: milliseconds(run.heartbeatMs),
        }),
    run.nodes === undefined
      ? undefined
      : t('inspector.extensions.budgets', {
          nodes: run.nodes,
          maximumNodes: nativeUiBudgets.maximumTreeNodes,
          updates: run.peakMutations ?? 0,
          maximumUpdates: nativeUiBudgets.maximumMutationsPerWindow,
          message: Math.ceil((run.largestMessageBytes ?? 0) / bytesPerKibibyte),
          maximumMessage:
            nativeUiBudgets.maximumMessageBytes / bytesPerKibibyte,
        }),
  ].filter(Boolean);
  return `${run.extensionName}: ${details.join(detailSeparator)}`;
};

const requestLine = (
  t: TFunction,
  formatTime: (at: number) => string,
  request: BrokerRequestRecord,
) =>
  t(
    request.outcome === brokerOutcomes.allowed
      ? 'inspector.extensions.requestAllowed'
      : 'inspector.extensions.requestDenied',
    {
      time: formatTime(request.at),
      extension: request.extensionName,
      contribution: request.contributionId,
      method: request.method,
      ms: milliseconds(request.durationMs),
      reason: request.reason,
    },
  );

/**
 * What extension UI is running in this browser and what the host did for
 * it: start times, failures, native budgets, broker requests and the
 * elements or props it kept out of a drawing. Extension code never sees the
 * reasons shown here.
 */
export const InspectorExtensionsPane = () => {
  const { t } = useTranslation();
  const { format } = useInstantFormat();
  const runs = useExtensionDiagnosticsStore((state) => state.runs);
  const requests = useExtensionDiagnosticsStore((state) => state.requests);
  const drops = useExtensionDiagnosticsStore((state) => state.drops);
  const clear = useExtensionDiagnosticsStore((state) => state.clear);
  const runList = Object.values(runs).sort((left, right) =>
    left.extensionName.localeCompare(right.extensionName),
  );
  return (
    <Box sx={{ display: 'grid', gap: 1 }}>
      <Box component="section" sx={{ display: 'grid', gap: 0.25 }}>
        <Typography component="h3" variant="subtitle2">
          {t('inspector.extensions.running')}
        </Typography>
        {runList.length === 0 ? (
          <Typography color="text.secondary" variant="body2">
            {t('inspector.extensions.noneRunning')}
          </Typography>
        ) : (
          runList.map((run) => (
            <Typography
              color={
                run.state === extensionRunStates.failed ? 'error' : undefined
              }
              key={run.key}
              variant="body2"
            >
              {runLine(t, run)}
            </Typography>
          ))
        )}
      </Box>
      <Box component="section" sx={{ display: 'grid', gap: 0.25 }}>
        <Typography component="h3" variant="subtitle2">
          {t('inspector.extensions.requests')}
        </Typography>
        {requests.length === 0 ? (
          <Typography color="text.secondary" variant="body2">
            {t('inspector.extensions.noRequests')}
          </Typography>
        ) : (
          requests.slice(0, shownRequests).map((request) => (
            <Typography
              color={
                request.outcome === brokerOutcomes.denied
                  ? 'warning.main'
                  : undefined
              }
              key={`${request.at}-${request.method}-${request.contributionId}`}
              sx={{ overflowWrap: 'anywhere' }}
              variant="body2"
            >
              {requestLine(t, (at) => format(at, requestTimeStyle), request)}
            </Typography>
          ))
        )}
      </Box>
      <Box component="section" sx={{ display: 'grid', gap: 0.25 }}>
        <Typography component="h3" variant="subtitle2">
          {t('inspector.extensions.dropped')}
        </Typography>
        {drops.length === 0 ? (
          <Typography color="text.secondary" variant="body2">
            {t('inspector.extensions.nothingDropped')}
          </Typography>
        ) : (
          drops.slice(0, shownDrops).map((drop) => (
            <Typography key={`${drop.at}-${drop.message}`} variant="body2">
              {t('inspector.extensions.drop', {
                extension: drop.extensionName,
                message: drop.message,
              })}
            </Typography>
          ))
        )}
      </Box>
      <Box sx={{ alignItems: 'center', display: 'flex', gap: 1 }}>
        <Typography color="text.secondary" sx={{ flex: 1 }} variant="caption">
          {t('inspector.extensions.privacy')}
        </Typography>
        <Button onClick={clear} size="small">
          {t('inspector.extensions.clear')}
        </Button>
      </Box>
    </Box>
  );
};
