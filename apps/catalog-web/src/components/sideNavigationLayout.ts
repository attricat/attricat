export const compactNavigationWidth = 88;
export const expandedNavigationWidth = 264;
export const managementSidebarWidth = 248;
export const navigationHeaderHeight = 64;
export const mobileExplorePanelId = 'mobile-explore-panel';

export const compactNavigationLabelSx = {
  fontSize: '0.65rem',
  lineHeight: 1.1,
  textAlign: 'center',
} as const;

export const compactNavigationItemSx = {
  '&.Mui-selected': {
    '& .MuiListItemIcon-root': { color: 'inherit' },
    color: 'primary.main',
  },
  borderRadius: 1.5,
  flexDirection: 'column',
  justifyContent: 'center',
  minHeight: navigationHeaderHeight,
  px: 0.5,
  width: '100%',
} as const;

export const navigationHeaderSx = {
  alignItems: 'center',
  display: 'flex',
  height: navigationHeaderHeight,
} as const;

export const isWithinRoute = (pathname: string, route: string) =>
  pathname === route || pathname.startsWith(`${route}/`);
