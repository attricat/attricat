import { useState } from 'react';
import { navigationRoutes } from './navigation';
import { isWithinRoute } from './sideNavigationLayout';

export const compactNavigationPanels = {
  explore: 'explore',
  extensions: 'extensions',
  manage: 'manage',
} as const;
type CompactNavigationPanel =
  (typeof compactNavigationPanels)[keyof typeof compactNavigationPanels];

// The panel presenting the section a route belongs to, if it has one.
export const compactNavigationPanelForRoute = (
  pathname: string,
): CompactNavigationPanel | undefined => {
  if (pathname === navigationRoutes.explore)
    return compactNavigationPanels.explore;
  if (isWithinRoute(pathname, navigationRoutes.extensionContributions))
    return compactNavigationPanels.extensions;
  if (isWithinRoute(pathname, navigationRoutes.manage))
    return compactNavigationPanels.manage;
  return undefined;
};

type PanelControl = {
  onOpenChange?: (open: boolean) => void;
  open?: boolean;
};

// Compact panels are mutually exclusive. Each panel may be controlled by the
// parent layout, which also needs to reserve width for the open pane.
export const useCompactNavigationPanels = (
  pathname: string,
  controls: Record<CompactNavigationPanel, PanelControl>,
) => {
  const [localOpen, setLocalOpen] = useState<
    Record<CompactNavigationPanel, boolean>
  >(() => {
    const routePanel = compactNavigationPanelForRoute(pathname);
    return {
      explore: routePanel === compactNavigationPanels.explore,
      extensions: routePanel === compactNavigationPanels.extensions,
      manage: routePanel === compactNavigationPanels.manage,
    };
  });
  const isOpen = (panel: CompactNavigationPanel) =>
    controls[panel].open ?? localOpen[panel];
  const setOpen = (panel: CompactNavigationPanel, open: boolean) => {
    setLocalOpen((current) => ({ ...current, [panel]: open }));
    controls[panel].onOpenChange?.(open);
  };
  const openOnly = (panel: CompactNavigationPanel) =>
    Object.values(compactNavigationPanels).forEach((item) =>
      setOpen(item, item === panel),
    );
  const closeAll = () =>
    Object.values(compactNavigationPanels).forEach((item) =>
      setOpen(item, false),
    );
  const toggle = (panel: CompactNavigationPanel) => {
    if (isOpen(panel)) setOpen(panel, false);
    else openOnly(panel);
  };

  return { closeAll, isOpen, openOnly, toggle };
};
