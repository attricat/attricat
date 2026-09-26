import ArrowBackIcon from '@mui/icons-material/ArrowBack';
import ChevronRightIcon from '@mui/icons-material/ChevronRight';
import LogoutOutlinedIcon from '@mui/icons-material/LogoutOutlined';
import DarkModeOutlinedIcon from '@mui/icons-material/DarkModeOutlined';
import LightModeOutlinedIcon from '@mui/icons-material/LightModeOutlined';
import { useQuery } from '@tanstack/react-query';
import { Link, useRouterState } from '@tanstack/react-router';
import { createElement, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { currentSession } from '../features/auth/api';
import { authQueryKeys } from '../features/auth/queryKeys';
import { ExtensionOutlet } from '../features/extensions/ExtensionOutlet';
import { listSidebarExploreNavigation } from '../features/workspace/api';
import { workspaceQueryKeys } from '../features/workspace/queryKeys';
import { useSetMobileExplorePanelTarget } from './mobileNavigationPanelContext';
import { QueryErrorNotice } from './QueryErrorNotice';
import { RouterListItemButton } from './RouterLink';
import {
  ExplorerIcon,
  AppsIcon,
  BrandIcon,
  ExplorerShortcutIcon,
  ManagementIcon,
} from './systemIcons';
import {
  getVisibleManagementNavigationItems,
  navigationRoutes,
  primaryNavigationItems,
  profileNavigationItem,
} from './navigation';
import {
  Box,
  Divider,
  IconButton,
  List,
  ListItemButton,
  ListItemIcon,
  ListItemText,
  ListSubheader,
  Tooltip,
  Typography,
  useTheme,
} from '@mui/material';
import { useColorMode } from '../app/colorMode';

export const compactNavigationWidth = 88;
export const expandedNavigationWidth = 264;
export const managementSidebarWidth = 248;
const navigationHeaderHeight = 64;

const compactNavigationLabelSx = {
  fontSize: '0.65rem',
  lineHeight: 1.1,
  textAlign: 'center',
};

type MobileNavigationSection = 'primary' | 'explore' | 'extensions' | 'manage';

const mobileSectionForPathname = (
  pathname: string,
): MobileNavigationSection => {
  if (
    pathname === navigationRoutes.manage ||
    pathname.startsWith(`${navigationRoutes.manage}/`)
  )
    return 'manage';
  if (pathname === navigationRoutes.explore) return 'explore';
  if (
    pathname === navigationRoutes.extensionContributions ||
    pathname.startsWith(`${navigationRoutes.extensionContributions}/`)
  )
    return 'extensions';
  return 'primary';
};

type SideNavigationProps = {
  compact?: boolean;
  compactManageOpen?: boolean;
  compactExploreOpen?: boolean;
  compactExtensionsOpen?: boolean;
  onCompactManageOpenChange?: (open: boolean) => void;
  onCompactExploreOpenChange?: (open: boolean) => void;
  onCompactExtensionsOpenChange?: (open: boolean) => void;
  onNavigate?: () => void;
  onSignOut?: () => void;
};

export const SideNavigation = ({
  compact = false,
  compactManageOpen,
  compactExploreOpen,
  compactExtensionsOpen,
  onCompactManageOpenChange,
  onCompactExploreOpenChange,
  onCompactExtensionsOpenChange,
  onNavigate,
  onSignOut,
}: SideNavigationProps) => {
  const { t } = useTranslation();
  const mode = useTheme().palette.mode;
  const setColorMode = useColorMode((state) => state.setPreference);
  const setMobileExplorePanelTarget = useSetMobileExplorePanelTarget();
  const { pathname, search } = useRouterState({
    select: (state) => state.location,
  });
  // Manual pane navigation applies only to the route where it was chosen.
  // Browser history and outside links take precedence when the route changes.
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
  const mobileSection =
    mobileNavigation.pathname === pathname
      ? mobileNavigation.section
      : mobileSectionForPathname(pathname);
  const setMobileSection = (section: MobileNavigationSection) =>
    setMobileNavigation({ pathname, section });
  const mobileSubNavigationOpen = mobileSection !== 'primary';
  const showPrimaryNavigation = compact || mobileSection === 'primary';
  const [localCompactManageOpen, setLocalCompactManageOpen] = useState(
    pathname.startsWith(`${navigationRoutes.manage}/`),
  );
  const [localCompactExploreOpen, setLocalCompactExploreOpen] = useState(
    pathname === navigationRoutes.explore,
  );
  const [localCompactExtensionsOpen, setLocalCompactExtensionsOpen] = useState(
    pathname === navigationRoutes.extensionContributions ||
      pathname.startsWith(`${navigationRoutes.extensionContributions}/`),
  );
  const isCompactManageOpen = compactManageOpen ?? localCompactManageOpen;
  const isCompactExploreOpen = compactExploreOpen ?? localCompactExploreOpen;
  const isCompactExtensionsOpen =
    compactExtensionsOpen ?? localCompactExtensionsOpen;
  const setCompactManageOpen = (open: boolean) => {
    setLocalCompactManageOpen(open);
    onCompactManageOpenChange?.(open);
  };
  const setCompactExplore = (open: boolean) => {
    setLocalCompactExploreOpen(open);
    onCompactExploreOpenChange?.(open);
    if (open) {
      setCompactExtensionsOpen(false);
      setCompactManageOpen(false);
    }
  };
  const setCompactExtensionsOpen = (open: boolean) => {
    setLocalCompactExtensionsOpen(open);
    onCompactExtensionsOpenChange?.(open);
    if (open) {
      setCompactExplore(false);
      setCompactManageOpen(false);
    }
  };
  const session = useQuery({
    queryKey: authQueryKeys.session(),
    queryFn: currentSession,
  });
  const pinnedExplore = useQuery({
    queryKey: workspaceQueryKeys.sidebarExploreNavigation(),
    queryFn: listSidebarExploreNavigation,
  });
  const managementItems = getVisibleManagementNavigationItems(
    session.data?.capabilities,
  );
  const itemSx = compact
    ? {
        '&.Mui-selected': {
          '& .MuiListItemIcon-root': { color: 'inherit' },
          color: 'primary.main',
        },
        borderRadius: 1.5,
        flexDirection: 'column',
        justifyContent: 'center',
        minHeight: 64,
        px: 0.5,
        width: '100%',
      }
    : undefined;

  return (
    <Box
      sx={{
        backgroundColor: compact ? 'background.default' : 'background.paper',
        display: 'flex',
        flexDirection: 'column',
        height: '100%',
        // Desktop compact navigation renders its Explore/Manage panes beside
        // the icon rail. Let those absolutely positioned panes use the extra
        // width allocated by AppLayout instead of clipping them at the rail.
        overflow: compact ? 'visible' : 'hidden',
        position: 'relative',
        width: compact ? compactNavigationWidth : expandedNavigationWidth,
      }}
    >
      <Box
        sx={
          compact
            ? {
                alignItems: 'center',
                display: 'flex',
                height: navigationHeaderHeight,
                justifyContent: 'center',
              }
            : mobileSubNavigationOpen
              ? {
                  alignItems: 'center',
                  display: 'flex',
                  height: navigationHeaderHeight,
                  px: 1,
                }
              : {
                  alignItems: 'center',
                  display: 'flex',
                  height: navigationHeaderHeight,
                  px: 3,
                }
        }
      >
        {!compact && mobileSubNavigationOpen ? (
          <>
            <IconButton
              aria-label={t('navigation.back')}
              onClick={() => setMobileSection('primary')}
            >
              <ArrowBackIcon />
            </IconButton>
            <Typography sx={{ ml: 1 }} variant="h6">
              {t(
                mobileSection === 'explore'
                  ? 'navigation.entityExplorer'
                  : mobileSection === 'extensions'
                    ? 'navigation.apps'
                    : 'navigation.manage',
              )}
            </Typography>
          </>
        ) : (
          <Box
            aria-label={t('app.attricat')}
            sx={{ alignItems: 'center', display: 'flex', gap: 1.5 }}
          >
            <BrandIcon variant={compact ? 'mark' : 'wordmark'} />
          </Box>
        )}
      </Box>
      <Divider />
      <List
        sx={{
          display: compact ? 'flex' : 'block',
          flexDirection: compact ? 'column' : undefined,
          flexGrow: compact ? 0 : 1,
          minHeight: 0,
          overflowY: compact ? 'visible' : 'auto',
          px: compact ? 0.5 : 1,
          py: 1.5,
          width: '100%',
        }}
      >
        {!compact && mobileSection === 'primary' && (
          <ListItemButton
            aria-controls="mobile-explore-panel"
            aria-expanded={false}
            aria-label={t('navigation.entityExplorer')}
            onClick={() => setMobileSection('explore')}
          >
            <ListItemIcon>
              <ExplorerIcon />
            </ListItemIcon>
            <ListItemText primary={t('navigation.entityExplorer')} />
            <ChevronRightIcon />
          </ListItemButton>
        )}
        {showPrimaryNavigation &&
          primaryNavigationItems
            .filter((item) => compact || item.to !== navigationRoutes.explore)
            .map((item) => (
              <Tooltip
                key={item.to}
                placement="right"
                title={compact ? t(item.labelKey) : ''}
              >
                <ListItemButton
                  aria-label={t(item.labelKey)}
                  component={Link}
                  onClick={() => {
                    if (compact && item.to === navigationRoutes.explore) {
                      setCompactExplore(!isCompactExploreOpen);
                      return;
                    }
                    if (compact) {
                      setCompactExtensionsOpen(false);
                      setCompactManageOpen(false);
                      setCompactExplore(false);
                    }
                    onNavigate?.();
                  }}
                  selected={
                    item.to === navigationRoutes.explore
                      ? pathname === item.to && !search.locked
                      : pathname.startsWith(item.to)
                  }
                  sx={itemSx}
                  to={item.to}
                >
                  <ListItemIcon sx={compact ? { minWidth: 0 } : undefined}>
                    {createElement(item.icon)}
                  </ListItemIcon>
                  {compact ? (
                    <Typography sx={compactNavigationLabelSx} variant="caption">
                      {t(item.labelKey)}
                    </Typography>
                  ) : (
                    <ListItemText primary={t(item.labelKey)} />
                  )}
                </ListItemButton>
              </Tooltip>
            ))}
        {(compact || mobileSection === 'explore') && (
          <Box sx={compact ? { width: '100%' } : { px: 1 }}>
            <QueryErrorNotice
              error={pinnedExplore.error}
              isRetrying={pinnedExplore.isFetching}
              onRetry={() => void pinnedExplore.refetch()}
            />
          </Box>
        )}
        {!compact && mobileSection === 'explore' && (
          <Box
            aria-label={t('navigation.entityExplorer')}
            component="nav"
            id="mobile-explore-panel"
          >
            <List disablePadding>
              <ListItemButton
                component={Link}
                onClick={onNavigate}
                selected={
                  pathname === navigationRoutes.explore && !search.locked
                }
                to={navigationRoutes.explore}
              >
                <ListItemIcon>
                  <ExplorerIcon />
                </ListItemIcon>
                <ListItemText primary={t('navigation.allEntities')} />
              </ListItemButton>
              {pinnedExplore.data?.length ? (
                <ListSubheader disableSticky>
                  {t('navigation.shortcuts')}
                </ListSubheader>
              ) : null}
              {pinnedExplore.data?.map((item) => (
                <RouterListItemButton
                  aria-label={item.blueprint_name}
                  key={item.blueprint_code}
                  onClick={onNavigate}
                  search={{ blueprint: item.blueprint_code, locked: true }}
                  selected={
                    pathname === navigationRoutes.explore &&
                    search.blueprint === item.blueprint_code
                  }
                  style={{ color: 'inherit', textDecoration: 'none' }}
                  to={navigationRoutes.explore}
                >
                  <ListItemIcon>
                    <ExplorerShortcutIcon />
                  </ListItemIcon>
                  <ListItemText primary={item.blueprint_name} />
                </RouterListItemButton>
              ))}
            </List>
            <Box ref={setMobileExplorePanelTarget} sx={{ mt: 2 }} />
          </Box>
        )}
        {compact && (
          <Tooltip placement="right" title={t('navigation.apps')}>
            <ListItemButton
              aria-expanded={isCompactExtensionsOpen}
              aria-label={t('navigation.apps')}
              component={Link}
              onClick={() => setCompactExtensionsOpen(true)}
              selected={
                pathname === navigationRoutes.extensionContributions ||
                pathname.startsWith(
                  `${navigationRoutes.extensionContributions}/`,
                )
              }
              sx={itemSx}
              to={navigationRoutes.extensionContributions}
            >
              <ListItemIcon sx={{ minWidth: 0 }}>
                <AppsIcon />
              </ListItemIcon>
              <Typography sx={compactNavigationLabelSx} variant="caption">
                {t('navigation.apps')}
              </Typography>
            </ListItemButton>
          </Tooltip>
        )}
        {!compact && mobileSection === 'primary' && (
          <ListItemButton
            aria-label={t('navigation.apps')}
            component={Link}
            onClick={() => setMobileSection('extensions')}
            sx={{ mt: 1 }}
            to={navigationRoutes.extensionContributions}
          >
            <ListItemIcon>
              <AppsIcon />
            </ListItemIcon>
            <ListItemText primary={t('navigation.apps')} />
            <ChevronRightIcon />
          </ListItemButton>
        )}
        {!compact && mobileSection === 'extensions' && (
          <ExtensionOutlet
            canBrowseExtensions={
              session.data?.capabilities?.extensions_read === true
            }
            navigationDisplay="all"
            onNavigate={onNavigate}
            outlet="navigation"
          />
        )}
        {compact && (
          <Tooltip placement="right" title={t('navigation.manage')}>
            <ListItemButton
              aria-expanded={isCompactManageOpen}
              aria-label={t('navigation.manage')}
              component={Link}
              onClick={() => {
                setCompactExtensionsOpen(false);
                setCompactExplore(false);
                setCompactManageOpen(true);
              }}
              selected={
                pathname === navigationRoutes.manage ||
                pathname.startsWith(`${navigationRoutes.manage}/`)
              }
              sx={itemSx}
              to={navigationRoutes.manage}
            >
              <ListItemIcon sx={{ minWidth: 0 }}>
                <ManagementIcon />
              </ListItemIcon>
              <Typography sx={compactNavigationLabelSx} variant="caption">
                {t('navigation.manage')}
              </Typography>
            </ListItemButton>
          </Tooltip>
        )}
        {!compact && mobileSection === 'primary' && (
          <ListItemButton
            aria-label={t('navigation.manage')}
            onClick={() => setMobileSection('manage')}
            sx={{ mt: 1 }}
          >
            <ListItemIcon>
              <ManagementIcon />
            </ListItemIcon>
            <ListItemText primary={t('navigation.manage')} />
            <ChevronRightIcon />
          </ListItemButton>
        )}
        {!compact && mobileSection === 'manage' && (
          <>
            <ListItemButton
              component={Link}
              onClick={onNavigate}
              selected={
                pathname === navigationRoutes.manage ||
                pathname === `${navigationRoutes.manage}/`
              }
              to={navigationRoutes.manage}
            >
              <ListItemIcon>
                <ManagementIcon />
              </ListItemIcon>
              <ListItemText primary={t('navigation.dashboard')} />
            </ListItemButton>
            {managementItems.map((item) => (
              <ListItemButton
                aria-label={t(item.labelKey)}
                component={Link}
                key={item.to}
                onClick={onNavigate}
                selected={pathname.startsWith(item.to)}
                to={item.to}
              >
                <ListItemIcon>{createElement(item.icon)}</ListItemIcon>
                <ListItemText primary={t(item.labelKey)} />
              </ListItemButton>
            ))}
          </>
        )}
      </List>
      {compact && isCompactManageOpen && (
        <Box
          aria-label={t('navigation.manage')}
          component="nav"
          sx={{
            backgroundColor: 'background.paper',
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
          <Box
            sx={{
              alignItems: 'center',
              display: 'flex',
              height: navigationHeaderHeight,
              px: 3,
            }}
          >
            <Typography variant="h6">{t('navigation.manage')}</Typography>
          </Box>
          <Divider />
          <List sx={{ px: 1, py: 1.5 }}>
            <ListItemButton
              component={Link}
              selected={
                pathname === navigationRoutes.manage ||
                pathname === `${navigationRoutes.manage}/`
              }
              to={navigationRoutes.manage}
            >
              <ListItemIcon>
                <ManagementIcon />
              </ListItemIcon>
              <ListItemText primary={t('navigation.dashboard')} />
            </ListItemButton>
            {managementItems.map((item) => (
              <ListItemButton
                component={Link}
                key={item.to}
                onClick={onNavigate}
                selected={pathname.startsWith(item.to)}
                to={item.to}
              >
                <ListItemIcon>{createElement(item.icon)}</ListItemIcon>
                <ListItemText primary={t(item.labelKey)} />
              </ListItemButton>
            ))}
          </List>
        </Box>
      )}
      {compact && isCompactExtensionsOpen && (
        <Box
          aria-label={t('navigation.apps')}
          component="nav"
          sx={{
            backgroundColor: 'background.paper',
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
          <Box
            sx={{
              alignItems: 'center',
              display: 'flex',
              height: navigationHeaderHeight,
              px: 3,
            }}
          >
            <Typography variant="h6">{t('navigation.apps')}</Typography>
          </Box>
          <Divider />
          <List sx={{ px: 1, py: 1.5 }}>
            <ExtensionOutlet
              canBrowseExtensions={
                session.data?.capabilities?.extensions_read === true
              }
              onBrowseExtensions={() => {
                setCompactExtensionsOpen(false);
                setCompactManageOpen(true);
              }}
              navigationDisplay="all"
              onNavigate={onNavigate}
              outlet="navigation"
            />
          </List>
        </Box>
      )}
      {compact && isCompactExploreOpen && (
        <Box
          aria-label={t('navigation.entityExplorer')}
          component="nav"
          sx={{
            backgroundColor: 'background.paper',
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
          <Box
            sx={{
              alignItems: 'center',
              display: 'flex',
              height: navigationHeaderHeight,
              px: 3,
            }}
          >
            <Typography variant="h6">
              {t('navigation.entityExplorer')}
            </Typography>
          </Box>
          <Divider />
          <List sx={{ px: 1, py: 1.5 }}>
            <ListItemButton
              component={Link}
              selected={pathname === navigationRoutes.explore && !search.locked}
              to={navigationRoutes.explore}
            >
              <ListItemIcon>
                <ExplorerIcon />
              </ListItemIcon>
              <ListItemText primary={t('navigation.allEntities')} />
            </ListItemButton>
            {pinnedExplore.data?.length ? (
              <ListSubheader disableSticky>
                {t('navigation.shortcuts')}
              </ListSubheader>
            ) : null}
            {pinnedExplore.data?.map((item) => (
              <RouterListItemButton
                key={item.blueprint_code}
                search={{ blueprint: item.blueprint_code, locked: true }}
                selected={
                  pathname === navigationRoutes.explore &&
                  search.blueprint === item.blueprint_code
                }
                style={{ color: 'inherit', textDecoration: 'none' }}
                to={navigationRoutes.explore}
              >
                <ListItemIcon>
                  <ExplorerShortcutIcon />
                </ListItemIcon>
                <ListItemText primary={item.blueprint_name} />
              </RouterListItemButton>
            ))}
            <Box ref={setMobileExplorePanelTarget} sx={{ mt: 2 }} />
          </List>
        </Box>
      )}
      {compact && <Box sx={{ flexGrow: 1 }} />}
      {(compact || mobileSection === 'primary') && (
        <>
          <Divider />
          <List
            sx={{
              display: compact ? 'flex' : 'block',
              flexDirection: compact ? 'column' : undefined,
              px: compact ? 0.5 : 1,
              py: 1.5,
            }}
          >
            {[profileNavigationItem].map((item) => (
              <Tooltip
                key={item.to}
                placement="right"
                title={compact ? t(item.labelKey) : ''}
              >
                <ListItemButton
                  aria-label={t(item.labelKey)}
                  component={Link}
                  onClick={() => {
                    if (compact) {
                      setCompactManageOpen(false);
                      setCompactExplore(false);
                      setCompactExtensionsOpen(false);
                    }
                    onNavigate?.();
                  }}
                  selected={pathname.startsWith(item.to)}
                  sx={itemSx}
                  to={item.to}
                >
                  <ListItemIcon sx={compact ? { minWidth: 0 } : undefined}>
                    {createElement(item.icon)}
                  </ListItemIcon>
                  {compact ? (
                    <Typography sx={compactNavigationLabelSx} variant="caption">
                      {t(item.labelKey)}
                    </Typography>
                  ) : (
                    <ListItemText primary={t(item.labelKey)} />
                  )}
                </ListItemButton>
              </Tooltip>
            ))}
            <Tooltip
              placement="right"
              title={
                compact
                  ? t(
                      mode === 'dark'
                        ? 'navigation.lightMode'
                        : 'navigation.darkMode',
                    )
                  : ''
              }
            >
              <ListItemButton
                aria-label={t(
                  mode === 'dark'
                    ? 'navigation.lightMode'
                    : 'navigation.darkMode',
                )}
                onClick={() => setColorMode(mode === 'dark' ? 'light' : 'dark')}
                sx={itemSx}
              >
                <ListItemIcon sx={compact ? { minWidth: 0 } : undefined}>
                  {mode === 'dark' ? (
                    <LightModeOutlinedIcon />
                  ) : (
                    <DarkModeOutlinedIcon />
                  )}
                </ListItemIcon>
                {compact ? (
                  <Typography sx={compactNavigationLabelSx} variant="caption">
                    {t(
                      mode === 'dark'
                        ? 'navigation.lightMode'
                        : 'navigation.darkMode',
                    )}
                  </Typography>
                ) : (
                  <ListItemText
                    primary={t(
                      mode === 'dark'
                        ? 'navigation.lightMode'
                        : 'navigation.darkMode',
                    )}
                  />
                )}
              </ListItemButton>
            </Tooltip>
            <Tooltip
              placement="right"
              title={compact ? t('navigation.signOut') : ''}
            >
              <ListItemButton
                aria-label={t('navigation.signOut')}
                onClick={onSignOut}
                sx={itemSx}
              >
                <ListItemIcon sx={compact ? { minWidth: 0 } : undefined}>
                  <LogoutOutlinedIcon />
                </ListItemIcon>
                {compact ? (
                  <Typography sx={compactNavigationLabelSx} variant="caption">
                    {t('navigation.signOut')}
                  </Typography>
                ) : (
                  <ListItemText primary={t('navigation.signOut')} />
                )}
              </ListItemButton>
            </Tooltip>
          </List>
        </>
      )}
    </Box>
  );
};
