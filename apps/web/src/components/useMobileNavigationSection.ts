import { useState } from 'react';
import { compactNavigationPanelForRoute } from './useCompactNavigationPanels';

type MobileNavigationSection = 'primary' | 'explore' | 'extensions' | 'manage';

export const mobileSectionTitleKeys = {
  explore: 'navigation.recordExplorer',
  extensions: 'navigation.apps',
  manage: 'navigation.manage',
} as const;

const mobileSectionForPathname = (pathname: string): MobileNavigationSection =>
  compactNavigationPanelForRoute(pathname) ?? 'primary';

// Manual pane navigation applies only to the route where it was chosen.
// Browser history and outside links take precedence when the route changes.
export const useMobileNavigationSection = (pathname: string) => {
  const [mobileNavigation, setMobileNavigation] = useState({
    pathname,
    section: mobileSectionForPathname(pathname),
  });
  if (mobileNavigation.pathname !== pathname) {
    setMobileNavigation({
      pathname,
      section: mobileSectionForPathname(pathname),
    });
  }
  const section =
    mobileNavigation.pathname === pathname
      ? mobileNavigation.section
      : mobileSectionForPathname(pathname);
  const setSection = (next: MobileNavigationSection) =>
    setMobileNavigation({ pathname, section: next });
  return [section, setSection] as const;
};
