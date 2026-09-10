import { InspectorIcon } from '../../components/system-icons';
import CloseIcon from '@mui/icons-material/Close';
import {
  Box,
  Fab,
  IconButton,
  Link,
  Paper,
  Tab,
  Tabs,
  Typography,
} from '@mui/material';
import { useQuery } from '@tanstack/react-query';
import { useEffect, useState, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { currentSession } from '../auth/api';
import { authQueryKeys } from '../auth/query-keys';
import { getApiHealth } from './api';
import { inspectorQueryKeys } from './query-keys';
import { recentTimings, subscribeTimings, type TimingEntry } from './timing';

type InspectorPane = {
  id: string;
  label: string;
  content: ReactNode;
};

const API_HEALTH_POLL_INTERVAL_MS = 5_000;
const API_RETRY_INTERVAL_MS = 1_000;
const INSPECTOR_STATE_STORAGE_KEY = 'catalog.inspector-expanded';

type InspectorState = {
  activePane: string;
  expanded: boolean;
};

const defaultInspectorState: InspectorState = {
  activePane: 'server',
  expanded: false,
};

const readInspectorState = (): InspectorState => {
  try {
    const value = localStorage.getItem(INSPECTOR_STATE_STORAGE_KEY);
    if (value === 'true') return { ...defaultInspectorState, expanded: true };
    if (!value) return defaultInspectorState;
    const state = JSON.parse(value) as Partial<InspectorState>;
    return {
      activePane:
        typeof state.activePane === 'string'
          ? state.activePane
          : defaultInspectorState.activePane,
      expanded:
        typeof state.expanded === 'boolean'
          ? state.expanded
          : defaultInspectorState.expanded,
    };
  } catch {
    return defaultInspectorState;
  }
};

export const Inspector = () => {
  const { t } = useTranslation();
  const [inspectorState, setInspectorState] = useState(readInspectorState);
  const [timings, setTimings] = useState<TimingEntry[]>(recentTimings);
  const { activePane, expanded } = inspectorState;
  useEffect(() => subscribeTimings(() => setTimings(recentTimings())), []);
  const updateInspectorState = (updates: Partial<InspectorState>) => {
    const nextState = { ...inspectorState, ...updates };
    setInspectorState(nextState);
    try {
      localStorage.setItem(
        INSPECTOR_STATE_STORAGE_KEY,
        JSON.stringify(nextState),
      );
    } catch {
      // The Inspector remains usable when storage is unavailable.
    }
  };
  const apiHealth = useQuery({
    queryKey: inspectorQueryKeys.apiHealth(),
    queryFn: getApiHealth,
    retry: false,
    refetchInterval: (query) =>
      query.state.status === 'error'
        ? API_RETRY_INTERVAL_MS
        : API_HEALTH_POLL_INTERVAL_MS,
  });
  const session = useQuery({
    queryKey: authQueryKeys.session(),
    queryFn: currentSession,
    retry: false,
  });
  const apiRestarting = apiHealth.isError;
  const apiChecking = apiHealth.isPending;
  const apiStatus = apiRestarting
    ? t('inspector.apiRestarting')
    : apiChecking
      ? t('inspector.checkingApi')
      : t('inspector.apiReady');
  const panes: InspectorPane[] = [
    {
      id: 'server',
      label: t('inspector.server'),
      content: (
        <Box sx={{ display: 'grid', gap: 0.5 }}>
          <Typography variant="body2">
            {t('inspector.apiStatus', {
              status: apiRestarting
                ? t('inspector.restarting')
                : apiChecking
                  ? t('inspector.checking')
                  : t('inspector.ready'),
            })}
          </Typography>
          <Typography color="text.secondary" variant="caption">
            {apiRestarting
              ? t('inspector.waitingForHealthCheck')
              : t('inspector.healthCheckInterval')}
          </Typography>
        </Box>
      ),
    },
    {
      id: 'performance',
      label: t('inspector.performance'),
      content: (
        <Box sx={{ display: 'grid', gap: 0.5 }}>
          {timings.length === 0 ? (
            <Typography color="text.secondary" variant="body2">
              {t('inspector.noTimings')}
            </Typography>
          ) : (
            timings.map((entry) => (
              <Typography
                key={`${entry.recordedAt}-${entry.phases[0]?.name}`}
                variant="body2"
              >
                {entry.phases
                  .map((phase) =>
                    phase.name.startsWith('sql')
                      ? `${phase.name === 'sql' ? 'SQL' : `SQL ${phase.name.slice(4).replaceAll('-', ' ')}`} (${phase.queryCount ?? '?'} queries): ${phase.duration.toFixed(2)} ms`
                      : `${phase.name}: ${phase.duration.toFixed(2)} ms`,
                  )
                  .join(' · ')}
              </Typography>
            ))
          )}
          <Typography color="text.secondary" variant="caption">
            {t('inspector.timingPrivacy')}
          </Typography>
        </Box>
      ),
    },
    {
      id: 'session',
      label: t('inspector.session'),
      content: (
        <Box sx={{ display: 'grid', gap: 0.5 }}>
          {session.isPending ? (
            <Typography variant="body2">
              {t('inspector.checkingSession')}
            </Typography>
          ) : session.data ? (
            <>
              <Typography variant="body2">
                {t('inspector.signedInAs', { email: session.data.email })}
              </Typography>
              <Typography color="text.secondary" variant="caption">
                {t('inspector.workspace', {
                  workspace: session.data.login_identifier,
                })}
              </Typography>
              <Typography color="text.secondary" variant="caption">
                {t('inspector.tokenHelp')}
              </Typography>
              <Link href="/profile#personal-api-tokens">
                {t('inspector.manageTokens')}
              </Link>
            </>
          ) : (
            <Typography variant="body2">
              {t('inspector.notSignedIn')}
            </Typography>
          )}
        </Box>
      ),
    },
  ];
  const pane = panes.find(({ id }) => id === activePane) ?? panes[0];
  const statusColor =
    apiRestarting || apiChecking ? 'warning.main' : 'success.main';

  return (
    <>
      {!expanded && (
        <Fab
          aria-label={t('inspector.open')}
          color="default"
          onClick={() => updateInspectorState({ expanded: true })}
          size="small"
          sx={{
            backgroundColor: 'rgba(255, 255, 255, 0.9)',
            bottom: 16,
            color: 'text.secondary',
            position: 'fixed',
            right: 16,
            zIndex: (theme) => theme.zIndex.modal + 1,
            '&:hover': {
              backgroundColor: 'rgba(21, 101, 192, 0.14)',
              boxShadow: 4,
              color: 'primary.main',
            },
          }}
        >
          <InspectorIcon fontSize="small" />
          <Box
            aria-label={apiStatus}
            role="status"
            sx={{
              backgroundColor: statusColor,
              border: 2,
              borderColor: 'background.paper',
              borderRadius: '50%',
              bottom: 3,
              height: 10,
              position: 'absolute',
              right: 3,
              width: 10,
            }}
          />
        </Fab>
      )}
      {expanded && (
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
              minHeight: 36,
              px: 1.5,
            }}
          >
            <Box
              aria-label={apiStatus}
              role="status"
              sx={{
                backgroundColor: statusColor,
                borderRadius: '50%',
                height: 8,
                width: 8,
              }}
            />
            <Typography
              aria-live="polite"
              sx={{ flexGrow: 1 }}
              variant="caption"
            >
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
              height: '20vh',
              overflow: 'auto',
              px: 1.5,
              py: 1,
            }}
          >
            <Tabs
              aria-label={t('inspector.tools')}
              onChange={(_, value: string) =>
                updateInspectorState({ activePane: value })
              }
              sx={{
                borderBottom: 1,
                borderColor: 'divider',
                minHeight: 28,
                '& .MuiTabs-indicator': { height: 2 },
              }}
              value={pane.id}
            >
              {panes.map(({ id, label }) => (
                <Tab
                  key={id}
                  label={label}
                  sx={{
                    fontSize: '0.75rem',
                    fontWeight: 600,
                    minHeight: 28,
                    py: 0,
                  }}
                  value={id}
                />
              ))}
            </Tabs>
            <Box sx={{ pt: 1 }}>{pane.content}</Box>
          </Box>
        </Paper>
      )}
    </>
  );
};
