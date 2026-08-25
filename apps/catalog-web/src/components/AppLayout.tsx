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
  Button,
  Drawer,
  IconButton,
  Toolbar,
  Typography,
  useMediaQuery,
  useTheme,
} from '@mui/material';
import { useState } from 'react';
import { currentSession, logout } from '../features/auth/api';
import { drawerWidth, SideNavigation } from './SideNavigation';

export const AppLayout = () => {
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
  const theme = useTheme();
  const isDesktop = useMediaQuery(theme.breakpoints.up('md'));
  const [mobileOpen, setMobileOpen] = useState(false);
  const closeMobileNavigation = () => setMobileOpen(false);
  const signOut = async () => {
    await logout();
    queryClient.setQueryData(['auth', 'session'], null);
    await navigate({ to: '/login' });
  };
  if (pathname === '/login') return <Outlet />;
  if (session.isPending) return null;
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
          <SideNavigation />
        </Drawer>
      ) : (
        <>
          <AppBar position="fixed">
            <Toolbar>
              <IconButton
                aria-label="Open navigation"
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
                Catalog
              </Typography>
              <Button color="inherit" onClick={signOut}>
                Sign out
              </Button>
            </Toolbar>
          </AppBar>
          <Drawer
            onClose={closeMobileNavigation}
            open={mobileOpen}
            slotProps={{ paper: { sx: { width: drawerWidth } } }}
            variant="temporary"
          >
            <SideNavigation onNavigate={closeMobileNavigation} />
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
        {isDesktop && (
          <Box sx={{ display: 'flex', justifyContent: 'flex-end', p: 1 }}>
            <Button onClick={signOut}>Sign out</Button>
          </Box>
        )}
        <Outlet />
      </Box>
    </Box>
  );
};
