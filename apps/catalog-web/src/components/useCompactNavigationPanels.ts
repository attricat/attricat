import { useState } from 'react';
import { navigationRoutes } from './navigation';
import { isWithinRoute } from './sideNavigationLayout';

const compactNavigationPanels = ['explore', 'extensions', 'manage'] as const;
export type CompactNavigationPanel = (typeof compactNavigationPanels)[number];

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
  >(() => ({
    explore: pathname === navigationRoutes.explore,
    extensions: isWithinRoute(
      pathname,
      navigationRoutes.extensionContributions,
    ),
    manage: pathname.startsWith(`${navigationRoutes.manage}/`),
  }));
  const isOpen = (panel: CompactNavigationPanel) =>
    controls[panel].open ?? localOpen[panel];
  const setOpen = (panel: CompactNavigationPanel, open: boolean) => {
    setLocalOpen((current) => ({ ...current, [panel]: open }));
    controls[panel].onOpenChange?.(open);
  };
  const openOnly = (panel: CompactNavigationPanel) =>
    compactNavigationPanels.forEach((item) => setOpen(item, item === panel));
  const closeAll = () =>
    compactNavigationPanels.forEach((item) => setOpen(item, false));
  const toggle = (panel: CompactNavigationPanel) => {
    if (isOpen(panel)) setOpen(panel, false);
    else openOnly(panel);
  };

  return { closeAll, isOpen, openOnly, toggle };
};
