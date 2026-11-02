import { Box, Paper, Tab, Tabs, Typography } from '@mui/material';
import { useTranslation } from 'react-i18next';
import {
  apiHealthColor,
  apiHealthSummaryKeys,
  type ApiHealthState,
} from './apiHealth';
import {
  headerStatusDotSize,
  inspectorHeaderMinHeight,
  inspectorPaneIds,
  inspectorPaneOrder,
  inspectorPanelHeight,
  inspectorTabMinHeight,
  type InspectorPaneId,
} from './constants';
import { InspectorLauncher } from './InspectorLauncher';
import { InspectorExtensionsPane } from './InspectorExtensionsPane';
import { InspectorPerformancePane } from './InspectorPerformancePane';
import { InspectorServerPane } from './InspectorServerPane';
import { InspectorSessionPane } from './InspectorSessionPane';
import { useApiHealth } from './useApiHealth';
import { useInspectorState } from './useInspectorState';
import { useTabAccessibility } from '../../components/useTabAccessibility';
import { CloseIconButton } from '../../components/CloseIconButton';

const paneLabelKeys = {
  performance: 'inspector.performance',
  server: 'inspector.server',
  session: 'inspector.session',
  extensions: 'inspector.extensions.title',
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
  if (pane === inspectorPaneIds.extensions) return <InspectorExtensionsPane />;
  return <InspectorServerPane apiHealth={apiHealth} />;
};

export const Inspector = () => {
  const { t } = useTranslation();
  const tabId = useTabAccessibility();
  const [{ activePane, expanded }, updateInspectorState] = useInspectorState();
  const apiHealth = useApiHealth();
  const apiStatus = t(apiHealthSummaryKeys[apiHealth]);

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
        <CloseIconButton
          aria-expanded
          label={t('inspector.collapse')}
          onClick={() => updateInspectorState({ expanded: false })}
        />
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
          {inspectorPaneOrder.map((pane, index) => (
            <Tab
              {...tabId.tab(index)}
              key={pane}
              label={t(paneLabelKeys[pane])}
              sx={{
                typography: 'caption',
                fontWeight: 'fontWeightBold',
                minHeight: inspectorTabMinHeight,
                py: 0,
              }}
              value={pane}
            />
          ))}
        </Tabs>
        <Box
          {...tabId.panel(inspectorPaneOrder.indexOf(activePane))}
          sx={{ pt: 1 }}
        >
          <InspectorPaneContent apiHealth={apiHealth} pane={activePane} />
        </Box>
      </Box>
    </Paper>
  );
};
