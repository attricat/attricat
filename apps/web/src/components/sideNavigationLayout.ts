export const compactNavigationWidth = 88;
export const expandedNavigationWidth = 264;
export const managementSidebarWidth = 248;
export const navigationHeaderHeight = 64;
export const navigationEdgeToggleSize = 24;
export const mobileExplorePanelId = 'mobile-explore-panel';

export const compactNavigationLabelSx = {
  fontSize: '0.65rem',
  lineHeight: 1.1,
  textAlign: 'center',
} as const;

export const compactNavigationItemSx = {
  // The small label keeps the text color: primary on the selected
  // background is below the contrast that text this size needs.
  '&.Mui-selected': {
    '& .MuiListItemIcon-root': { color: 'primary.main' },
    color: 'text.primary',
  },
  borderRadius: 1.5,
  flexDirection: 'column',
  justifyContent: 'center',
  minHeight: navigationHeaderHeight,
  px: 0.5,
  width: '100%',
} as const;

/**
 * Hover for unselected items on the navigation background. The hover changes
 * instantly: MUI's background fade would cross-fade two adjacent items as the
 * pointer moves between them, so neither looks hovered for a moment.
 */
export const navigationItemHoverSx = {
  '& .MuiListItemButton-root': { transition: 'none' },
  '& .MuiListItemButton-root:not(.Mui-selected):hover': {
    backgroundColor: 'action.navigationHover',
  },
} as const;

export const navigationHeaderSx = {
  alignItems: 'center',
  display: 'flex',
  height: navigationHeaderHeight,
} as const;

export const isWithinRoute = (pathname: string, route: string) =>
  pathname === route || pathname.startsWith(`${route}/`);
