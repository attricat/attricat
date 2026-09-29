import { Drawer } from '@mui/material';
import { useState } from 'react';
import { navigationRoutes } from './navigation';
import { SideNavigation } from './SideNavigation';
import {
  compactNavigationWidth,
  isWithinRoute,
  managementSidebarWidth,
} from './sideNavigationLayout';

export const DesktopNavigation = ({
  onSignOut,
  pathname,
}: {
  onSignOut: () => void;
  pathname: string;
}) => {
  const [manageOpen, setManageOpen] = useState(
    isWithinRoute(pathname, navigationRoutes.manage),
  );
  const [exploreOpen, setExploreOpen] = useState(
    pathname === navigationRoutes.explore,
  );
  const [extensionsOpen, setExtensionsOpen] = useState(
    isWithinRoute(pathname, navigationRoutes.extensionContributions),
  );
  const width =
    compactNavigationWidth +
    (manageOpen || exploreOpen || extensionsOpen ? managementSidebarWidth : 0);

  return (
    <Drawer
      open
      sx={{ flexShrink: 0, width }}
      slotProps={{ paper: { sx: { overflow: 'hidden', width } } }}
      variant="permanent"
    >
      <SideNavigation
        compact
        compactExploreOpen={exploreOpen}
        compactExtensionsOpen={extensionsOpen}
        compactManageOpen={manageOpen}
        onCompactExploreOpenChange={setExploreOpen}
        onCompactExtensionsOpenChange={setExtensionsOpen}
        onCompactManageOpenChange={setManageOpen}
        onSignOut={onSignOut}
      />
    </Drawer>
  );
};
