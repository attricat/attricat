import MenuIcon from '@mui/icons-material/Menu';
import {
  Navigate,
  Outlet,
  useNavigate,
  useRouterState,
} from '@tanstack/react-router';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import {
  Alert,
  AppBar,
  Box,
  Button,
  Drawer,
  IconButton,
  Toolbar,
  Typography,
  CircularProgress,
  useMediaQuery,
  useTheme,
} from '@mui/material';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { currentSession, logout } from '../features/auth/api';
import { authQueryKeys } from '../features/auth/query-keys';
import { MobileNavigationPanelProvider } from './MobileNavigationPanel';
import { navigationRoutes } from './navigation';
import {
  compactNavigationWidth,
  expandedNavigationWidth,
  managementSidebarWidth,
  SideNavigation,
} from './SideNavigation';

export const SessionErrorState = ({
  onRetry,
  onSignOut,
}: {
  onRetry: () => void;
  onSignOut: () => void;
}) => {
  const { t } = useTranslation();

  return (
    <Box
      sx={{
        alignItems: 'center',
        display: 'flex',
        height: '100dvh',
        justifyContent: 'center',
        p: 3,
      }}
    >
      <Alert
        action={
          <Box sx={{ display: 'flex', gap: 1 }}>
            <Button color="inherit" onClick={onRetry} size="small">
              {t('auth.retrySession')}
            </Button>
            <Button color="inherit" onClick={onSignOut} size="small">
              {t('navigation.signOut')}
            </Button>
          </Box>
        }
        role="alert"
        severity="error"
      >
        {t('auth.sessionCheckFailed')}
      </Alert>
    </Box>
  );
};

export const SignOutErrorState = ({ onRetry }: { onRetry: () => void }) => {
  const { t } = useTranslation();

  return (
    <Alert
      action={
        <Button color="inherit" onClick={onRetry} size="small">
          {t('auth.retrySignOut')}
        </Button>
      }
      role="alert"
      severity="error"
    >
      {t('auth.signOutFailed')}
    </Alert>
  );
};

export const AppLayout = () => {
  const { t } = useTranslation();
  const pathname = useRouterState({
    select: (state) => state.location.pathname,
  });
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const session = useQuery({
    queryKey: authQueryKeys.session(),
    queryFn: currentSession,
    retry: false,
  });
  const theme = useTheme();
  const isDesktop = useMediaQuery(theme.breakpoints.up('md'));
  const [mobileOpen, setMobileOpen] = useState(false);
  const [desktopManageOpen, setDesktopManageOpen] = useState(
    pathname === navigationRoutes.manage ||
      pathname.startsWith(`${navigationRoutes.manage}/`),
  );
  const [signOutError, setSignOutError] = useState(false);
  const closeMobileNavigation = () => setMobileOpen(false);
  const completeSignOut = async () => {
    queryClient.clear();
    queryClient.setQueryData(authQueryKeys.session(), null);
    await navigate({ to: '/login' });
  };
  const signOut = async () => {
    try {
      await logout();
    } catch {
      setSignOutError(true);
      return;
    }
    await completeSignOut();
  };
  const signOutAfterSessionFailure = async () => {
    try {
      await logout();
    } catch {
      // The login route remains available even if the invalid session cannot be cleared.
    }
    await completeSignOut();
  };
  const isLoginRoute = pathname === '/login' || pathname.startsWith('/login/');
  if (
    pathname === '/password-reset' ||
    pathname.startsWith('/password-reset/') ||
    pathname === '/onboarding'
  )
    return <Outlet />;
  if (session.isPending)
    return (
      <Box
        aria-label={t('app.loading')}
        sx={{
          alignItems: 'center',
          display: 'flex',
          height: '100dvh',
          justifyContent: 'center',
        }}
      >
        <CircularProgress enableTrackSlot />
      </Box>
    );
  if (isLoginRoute)
    return session.data ? (
      <Navigate to={navigationRoutes.explore} />
    ) : (
      <Outlet />
    );
  if (session.isError)
    return (
      <SessionErrorState
        onRetry={() => void session.refetch()}
        onSignOut={() => void signOutAfterSessionFailure()}
      />
    );
  if (!session.data) {
    sessionStorage.setItem(
      'catalog.return-to',
      `${window.location.pathname}${window.location.search}${window.location.hash}`,
    );
    return <Navigate to="/login" />;
  }

  return (
    <MobileNavigationPanelProvider>
      <Box sx={{ display: 'flex', minHeight: '100dvh' }}>
        {signOutError && (
          <Box
            sx={{
              left: 24,
              maxWidth: 480,
              position: 'fixed',
              top: 24,
              zIndex: (theme) => theme.zIndex.snackbar,
            }}
          >
            <SignOutErrorState onRetry={() => void signOut()} />
          </Box>
        )}
        {isDesktop ? (
          <Drawer
            open
            sx={{
              flexShrink: 0,
              width:
                compactNavigationWidth +
                (desktopManageOpen ? managementSidebarWidth : 0),
            }}
            slotProps={{
              paper: {
                sx: {
                  overflow: 'hidden',
                  width:
                    compactNavigationWidth +
                    (desktopManageOpen ? managementSidebarWidth : 0),
                },
              },
            }}
            variant="permanent"
          >
            <SideNavigation
              compact
              compactManageOpen={desktopManageOpen}
              onCompactManageOpenChange={setDesktopManageOpen}
              onSignOut={signOut}
            />
          </Drawer>
        ) : (
          <>
            <AppBar position="fixed">
              <Toolbar>
                <IconButton
                  aria-label={t('navigation.open')}
                  color="inherit"
                  edge="start"
                  onClick={() => setMobileOpen(true)}
                >
                  <MenuIcon />
                </IconButton>
                <Typography
                  component="div"
                  sx={{ flexGrow: 1, ml: 1 }}
                  variant="h6"
                >
                  {t('app.attricat')}
                </Typography>
              </Toolbar>
            </AppBar>
            <Drawer
              onClose={closeMobileNavigation}
              open={mobileOpen}
              slotProps={{ paper: { sx: { width: expandedNavigationWidth } } }}
              variant="temporary"
            >
              <SideNavigation
                key={`${pathname}:${mobileOpen ? 'open' : 'closed'}`}
                onNavigate={closeMobileNavigation}
                onSignOut={signOut}
              />
            </Drawer>
          </>
        )}
        <Box
          component="main"
          sx={{
            flexGrow: 1,
            minWidth: 0,
            pt: { xs: 7, md: 0 },
          }}
        >
          <Outlet />
        </Box>
      </Box>
    </MobileNavigationPanelProvider>
  );
};
