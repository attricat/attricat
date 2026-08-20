import MenuIcon from '@mui/icons-material/Menu';
import { Outlet } from '@tanstack/react-router';
import {
  AppBar,
  Box,
  Drawer,
  IconButton,
  Toolbar,
  Typography,
  useMediaQuery,
  useTheme,
} from '@mui/material';
import { useState } from 'react';
import { drawerWidth, SideNavigation } from './SideNavigation';

export const AppLayout = () => {
  const theme = useTheme();
  const isDesktop = useMediaQuery(theme.breakpoints.up('md'));
  const [mobileOpen, setMobileOpen] = useState(false);
  const closeMobileNavigation = () => setMobileOpen(false);

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
              <Typography component="div" sx={{ ml: 1 }} variant="h6">
                Catalog
              </Typography>
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
        <Outlet />
      </Box>
    </Box>
  );
};
