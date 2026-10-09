import { Box, Divider, List, Typography } from '@mui/material';
import type { ReactNode } from 'react';
import {
  compactNavigationWidth,
  managementSidebarWidth,
  navigationHeaderSx,
} from './sideNavigationLayout';

// Desktop compact navigation pane rendered beside the icon rail.
export const NavigationFlyout = ({
  children,
  title,
}: {
  children: ReactNode;
  title: string;
}) => (
  <Box
    aria-label={title}
    component="nav"
    sx={{
      backgroundColor: 'background.navigation.panel',
      borderColor: 'divider',
      borderRight: 1,
      boxShadow: 3,
      height: '100%',
      left: compactNavigationWidth,
      overflowY: 'auto',
      position: 'absolute',
      top: 0,
      width: managementSidebarWidth,
      zIndex: 1,
    }}
  >
    <Box sx={{ ...navigationHeaderSx, px: 3 }}>
      <Typography variant="h6">{title}</Typography>
    </Box>
    <Divider />
    <List sx={{ px: 1, py: 1.5 }}>{children}</List>
  </Box>
);
