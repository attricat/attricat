import CloseIcon from '@mui/icons-material/Close';
import { Box, IconButton, Paper, Tab, Tabs, Typography } from '@mui/material';
import { useQuery } from '@tanstack/react-query';
import { useId } from 'react';
import { useTranslation } from 'react-i18next';
import { getApiHealth } from './api';
import {
  apiHealthColor,
  apiHealthSummaryKeys,
  type ApiHealthState,
} from './apiHealth';
import {
  API_HEALTH_POLL_INTERVAL_MS,
  API_RETRY_INTERVAL_MS,
  headerStatusDotSize,
  inspectorHeaderMinHeight,
  inspectorPaneIds,
  inspectorPaneOrder,
  inspectorPanelHeight,
  inspectorTabMinHeight,
  type InspectorPaneId,
} from './constants';
import { InspectorLauncher } from './InspectorLauncher';
import { InspectorPerformancePane } from './InspectorPerformancePane';
import { InspectorServerPane } from './InspectorServerPane';
import { InspectorSessionPane } from './InspectorSessionPane';
import { inspectorQueryKeys } from './queryKeys';
import { useInspectorState } from './useInspectorState';

const paneLabelKeys = {
  performance: 'inspector.performance',
  server: 'inspector.server',
  session: 'inspector.session',
} as const satisfies Record<InspectorPaneId, string>;

const InspectorPaneContent = ({
  apiHealth,
  pane,
}: {
  apiHealth: ApiHealthState;
  pane: InspectorPaneId;
}) => {
  if (pane === inspectorPaneIds.performance)
    return <InspectorPerformancePane />;
  if (pane === inspectorPaneIds.session) return <InspectorSessionPane />;
  return <InspectorServerPane apiHealth={apiHealth} />;
};

export const Inspector = () => {
  const { t } = useTranslation();
  const tabId = useId();
  const [{ activePane, expanded }, updateInspectorState] = useInspectorState();
  const apiHealthQuery = useQuery({
    queryKey: inspectorQueryKeys.apiHealth(),
    queryFn: getApiHealth,
    retry: false,
    refetchInterval: (query) =>
      query.state.status === 'error'
        ? API_RETRY_INTERVAL_MS
        : API_HEALTH_POLL_INTERVAL_MS,
  });
  const apiHealth: ApiHealthState = apiHealthQuery.isError
    ? 'restarting'
    : apiHealthQuery.isPending
      ? 'checking'
      : 'ready';
  const apiStatus = t(apiHealthSummaryKeys[apiHealth]);
  const tabIds = (pane: InspectorPaneId) => ({
    panel: `${tabId}-${pane}-panel`,
    tab: `${tabId}-${pane}-tab`,
  });

  if (!expanded)
    return (
      <InspectorLauncher
        apiHealth={apiHealth}
        onOpen={() => updateInspectorState({ expanded: true })}
      />
    );

  return (
    <Paper
      component="aside"
      elevation={8}
      sx={{
        bottom: 0,
        left: 0,
        position: 'fixed',
        right: 0,
        zIndex: (theme) => theme.zIndex.modal + 1,
      }}
    >
      <Box
        sx={{
          alignItems: 'center',
          display: 'flex',
          gap: 1,
          minHeight: inspectorHeaderMinHeight,
          px: 1.5,
        }}
      >
        <Box
          aria-label={apiStatus}
          role="status"
          sx={{
            backgroundColor: apiHealthColor(apiHealth),
            borderRadius: '50%',
            height: headerStatusDotSize,
            width: headerStatusDotSize,
          }}
        />
        <Typography aria-live="polite" sx={{ flexGrow: 1 }} variant="caption">
          {t('inspector.status', { status: apiStatus })}
        </Typography>
        <IconButton
          aria-expanded
          aria-label={t('inspector.collapse')}
          onClick={() => updateInspectorState({ expanded: false })}
          size="small"
        >
          <CloseIcon fontSize="small" />
        </IconButton>
      </Box>
      <Box
        sx={{
          borderTop: 1,
          borderColor: 'divider',
          boxSizing: 'border-box',
          height: inspectorPanelHeight,
          overflow: 'auto',
          px: 1.5,
          py: 1,
        }}
      >
        <Tabs
          aria-label={t('inspector.tools')}
          onChange={(_, value: InspectorPaneId) =>
            updateInspectorState({ activePane: value })
          }
          sx={{
            borderBottom: 1,
            borderColor: 'divider',
            minHeight: inspectorTabMinHeight,
            '& .MuiTabs-indicator': { height: 2 },
          }}
          value={activePane}
        >
          {inspectorPaneOrder.map((pane) => (
            <Tab
              aria-controls={tabIds(pane).panel}
              id={tabIds(pane).tab}
              key={pane}
              label={t(paneLabelKeys[pane])}
              sx={{
                fontSize: '0.75rem',
                fontWeight: 600,
                minHeight: inspectorTabMinHeight,
                py: 0,
              }}
              value={pane}
            />
          ))}
        </Tabs>
        <Box
          aria-labelledby={tabIds(activePane).tab}
          id={tabIds(activePane).panel}
          role="tabpanel"
          sx={{ pt: 1 }}
        >
          <InspectorPaneContent apiHealth={apiHealth} pane={activePane} />
        </Box>
      </Box>
    </Paper>
  );
};
