import { Drawer } from '@mui/material';
import { useState } from 'react';
import { NavigationEdgeToggle } from './NavigationEdgeToggle';
import {
  saveNavigationPanelCollapsed,
  savedNavigationPanelCollapsed,
} from './navigationPanelPreference';
import { SideNavigation } from './SideNavigation';
import {
  compactNavigationWidth,
  managementSidebarWidth,
} from './sideNavigationLayout';
import { compactNavigationPanelForRoute } from './useCompactNavigationPanels';

export const DesktopNavigation = ({
  onSignOut,
  pathname,
}: {
  onSignOut: () => void;
  pathname: string;
}) => {
  const [initialPanel] = useState(() =>
    savedNavigationPanelCollapsed()
      ? undefined
      : compactNavigationPanelForRoute(pathname),
  );
  const [manageOpen, setManageOpen] = useState(initialPanel === 'manage');
  const [exploreOpen, setExploreOpen] = useState(initialPanel === 'explore');
  const [extensionsOpen, setExtensionsOpen] = useState(
    initialPanel === 'extensions',
  );
  // Opening any panel from the rail forgets an earlier collapse.
  const rememberingOpen =
    (setOpen: (open: boolean) => void) => (open: boolean) => {
      if (open) saveNavigationPanelCollapsed(false);
      setOpen(open);
    };
  const panelOpen = manageOpen || exploreOpen || extensionsOpen;
  const routePanel = compactNavigationPanelForRoute(pathname);
  const width =
    compactNavigationWidth + (panelOpen ? managementSidebarWidth : 0);
  const togglePanel = () => {
    if (panelOpen) {
      setManageOpen(false);
      setExploreOpen(false);
      setExtensionsOpen(false);
      saveNavigationPanelCollapsed(true);
      return;
    }
    saveNavigationPanelCollapsed(false);
    if (routePanel === 'manage') setManageOpen(true);
    if (routePanel === 'explore') setExploreOpen(true);
    if (routePanel === 'extensions') setExtensionsOpen(true);
  };

  return (
    <>
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
          onCompactExploreOpenChange={rememberingOpen(setExploreOpen)}
          onCompactExtensionsOpenChange={rememberingOpen(setExtensionsOpen)}
          onCompactManageOpenChange={rememberingOpen(setManageOpen)}
          onSignOut={onSignOut}
        />
      </Drawer>
      {(panelOpen || routePanel) && (
        <NavigationEdgeToggle
          edge={width}
          expanded={panelOpen}
          onToggle={togglePanel}
        />
      )}
    </>
  );
};
