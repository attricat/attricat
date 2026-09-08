import MenuIcon from '@mui/icons-material/Menu';
import {
  Navigate,
  Outlet,
  useNavigate,
  useRouterState,
} from '@tanstack/react-router';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import {
  AppBar,
  Box,
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
import { listSidebarExploreNavigation } from '../features/workspace/api';
import { workspaceQueryKeys } from '../features/workspace/query-keys';
import { drawerWidth, SideNavigation } from './SideNavigation';

export const AppLayout = () => {
  const { t } = useTranslation();
  const pathname = useRouterState({
    select: (state) => state.location.pathname,
  });
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const session = useQuery({
    queryKey: ['auth', 'session'],
    queryFn: currentSession,
    retry: false,
  });
  const workspaceNavigation = useQuery({
    queryKey: workspaceQueryKeys.sidebarExploreNavigation(),
    queryFn: listSidebarExploreNavigation,
    enabled: Boolean(session.data),
  });
  const theme = useTheme();
  const isDesktop = useMediaQuery(theme.breakpoints.up('md'));
  const [mobileOpen, setMobileOpen] = useState(false);
  const closeMobileNavigation = () => setMobileOpen(false);
  const signOut = async () => {
    await logout();
    queryClient.clear();
    queryClient.setQueryData(['auth', 'session'], null);
    await navigate({ to: '/login' });
  };
  if (
    pathname === '/login' ||
    pathname.startsWith('/login/') ||
    pathname === '/password-reset' ||
    pathname.startsWith('/password-reset/') ||
    pathname === '/onboarding'
  )
    return <Outlet />;
  if (
    session.isPending ||
    (Boolean(session.data) && workspaceNavigation.isPending)
  )
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
        <CircularProgress />
      </Box>
    );
  if (!session.data) {
    sessionStorage.setItem(
      'catalog.return-to',
      `${window.location.pathname}${window.location.search}${window.location.hash}`,
    );
    return <Navigate to="/login" />;
  }

  return (
    <Box sx={{ display: 'flex', minHeight: '100dvh' }}>
      {isDesktop ? (
        <Drawer
          open
          sx={{ flexShrink: 0, width: drawerWidth }}
          slotProps={{ paper: { sx: { width: drawerWidth } } }}
          variant="permanent"
        >
          <SideNavigation onSignOut={signOut} />
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
            slotProps={{ paper: { sx: { width: drawerWidth } } }}
            variant="temporary"
          >
            <SideNavigation
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
  );
};
