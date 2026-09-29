import { useState } from 'react';
import { navigationRoutes } from './navigation';
import { isWithinRoute } from './sideNavigationLayout';

export type MobileNavigationSection =
  'primary' | 'explore' | 'extensions' | 'manage';

export const mobileSectionTitleKeys = {
  explore: 'navigation.entityExplorer',
  extensions: 'navigation.apps',
  manage: 'navigation.manage',
} as const;

const mobileSectionForPathname = (
  pathname: string,
): MobileNavigationSection => {
  if (isWithinRoute(pathname, navigationRoutes.manage)) return 'manage';
  if (pathname === navigationRoutes.explore) return 'explore';
  if (isWithinRoute(pathname, navigationRoutes.extensionContributions))
    return 'extensions';
  return 'primary';
};

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
