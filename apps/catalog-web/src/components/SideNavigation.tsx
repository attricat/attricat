import AssessmentOutlinedIcon from '@mui/icons-material/AssessmentOutlined';
import BookmarkBorderOutlinedIcon from '@mui/icons-material/BookmarkBorderOutlined';
import BoltOutlinedIcon from '@mui/icons-material/BoltOutlined';
import CategoryOutlinedIcon from '@mui/icons-material/CategoryOutlined';
import ChevronRightIcon from '@mui/icons-material/ChevronRight';
import ExpandMoreIcon from '@mui/icons-material/ExpandMore';
import FactCheckOutlinedIcon from '@mui/icons-material/FactCheckOutlined';
import FolderOutlinedIcon from '@mui/icons-material/FolderOutlined';
import LogoutOutlinedIcon from '@mui/icons-material/LogoutOutlined';
import ManageAccountsOutlinedIcon from '@mui/icons-material/ManageAccountsOutlined';
import PersonOutlinedIcon from '@mui/icons-material/PersonOutlined';
import SmartToyOutlinedIcon from '@mui/icons-material/SmartToyOutlined';
import SettingsOutlinedIcon from '@mui/icons-material/SettingsOutlined';
import TravelExploreOutlinedIcon from '@mui/icons-material/TravelExploreOutlined';
import { useQuery } from '@tanstack/react-query';
import { Link, useRouterState } from '@tanstack/react-router';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { currentSession } from '../features/auth/api';
import { authQueryKeys } from '../features/auth/query-keys';
import { ExtensionOutlet } from '../features/extensions/ExtensionOutlet';
import { listSidebarExploreNavigation } from '../features/workspace/api';
import { workspaceQueryKeys } from '../features/workspace/query-keys';
import { QueryErrorNotice } from './QueryErrorNotice';
import { RouterListItemButton } from './RouterLink';
import {
  Box,
  Collapse,
  Divider,
  List,
  ListItemButton,
  ListItemIcon,
  ListItemText,
  ListSubheader,
  Tooltip,
  Typography,
} from '@mui/material';

const navigationItems = [
  {
    icon: <TravelExploreOutlinedIcon />,
    labelKey: 'navigation.entityExplorer',
    to: '/',
  },
  {
    icon: <CategoryOutlinedIcon />,
    labelKey: 'navigation.blueprints',
    to: '/manage/blueprints',
  },
  {
    icon: <FolderOutlinedIcon />,
    labelKey: 'navigation.contexts',
    to: '/manage/contexts',
  },
  {
    icon: <SmartToyOutlinedIcon />,
    labelKey: 'navigation.agents',
    to: '/agents',
  },
  {
    icon: <AssessmentOutlinedIcon />,
    labelKey: 'navigation.dataHealth',
    to: '/manage/data-health',
  },
  {
    icon: <FactCheckOutlinedIcon />,
    labelKey: 'navigation.auditLog',
    to: '/manage/audit-log',
  },
  {
    icon: <BoltOutlinedIcon />,
    labelKey: 'navigation.extensions',
    to: '/manage/extensions',
  },
  {
    icon: <PersonOutlinedIcon />,
    labelKey: 'navigation.profile',
    to: '/profile',
  },
  {
    icon: <ManageAccountsOutlinedIcon />,
    labelKey: 'navigation.workspaceManagement',
    to: '/manage/workspace/members',
  },
] as const;

export const drawerWidth = 88;
export const managementSidebarWidth = 248;

type SideNavigationProps = {
  compact?: boolean;
  compactManageOpen?: boolean;
  onCompactManageOpenChange?: (open: boolean) => void;
  onNavigate?: () => void;
  onSignOut?: () => void;
};

export const SideNavigation = ({
  compact = false,
  compactManageOpen,
  onCompactManageOpenChange,
  onNavigate,
  onSignOut,
}: SideNavigationProps) => {
  const { t } = useTranslation();
  const { pathname, search } = useRouterState({
    select: (state) => state.location,
  });
  const [manageOpen, setManageOpen] = useState(false);
  const [localCompactManageOpen, setLocalCompactManageOpen] = useState(
    pathname.startsWith('/manage/'),
  );
  const [compactExploreOpen, setCompactExploreOpen] = useState(
    pathname === '/',
  );
  const isCompactManageOpen = compactManageOpen ?? localCompactManageOpen;
  const setCompactManageOpen = (open: boolean) => {
    setLocalCompactManageOpen(open);
    onCompactManageOpenChange?.(open);
  };
  const setCompactExplore = (open: boolean) => {
    setCompactExploreOpen(open);
    if (open) setCompactManageOpen(false);
    onCompactManageOpenChange?.(open);
  };
  const isManageOpen = manageOpen || pathname.startsWith('/manage/');
  const session = useQuery({
    queryKey: authQueryKeys.session(),
    queryFn: currentSession,
  });
  const pinnedExplore = useQuery({
    queryKey: workspaceQueryKeys.sidebarExploreNavigation(),
    queryFn: listSidebarExploreNavigation,
  });
  const canManageWorkspace = Boolean(
    session.data?.capabilities?.members_manage ||
    session.data?.capabilities?.roles_manage ||
    session.data?.capabilities?.tokens_manage,
  );
  const visibleNavigationItems = navigationItems.filter(
    (item) =>
      (item.to !== '/manage/workspace/members' || canManageWorkspace) &&
      (item.to !== '/manage/audit-log' ||
        session.data?.capabilities?.audit_read) &&
      (item.to !== '/manage/extensions' ||
        session.data?.capabilities?.extensions_read),
  );
  const primaryItems = visibleNavigationItems.filter(
    (item) => item.to === '/' || item.to === '/agents',
  );
  const managementItems = visibleNavigationItems.filter(
    (item) =>
      item.to !== '/' && item.to !== '/agents' && item.to !== '/profile',
  );
  const itemSx = compact
    ? {
        '&.Mui-selected': {
          '& .MuiListItemIcon-root': { color: 'inherit' },
          '&:hover': { backgroundColor: 'primary.dark' },
          backgroundColor: 'primary.main',
          color: 'primary.contrastText',
        },
        borderRadius: 1.5,
        flexDirection: 'column',
        justifyContent: 'center',
        minHeight: 64,
        mx: 0.5,
        px: 0.5,
      }
    : undefined;

  return (
    <Box
      sx={{
        backgroundColor: compact ? 'background.default' : 'background.paper',
        display: 'flex',
        flexDirection: 'column',
        height: '100%',
        position: 'relative',
        width: drawerWidth,
      }}
    >
      <Box
        sx={
          compact
            ? {
                alignItems: 'center',
                display: 'flex',
                height: 64,
                justifyContent: 'center',
              }
            : { px: 3, py: 2 }
        }
      >
        <Typography
          aria-label={t('app.attricat')}
          color="primary"
          sx={{ fontWeight: 700 }}
          variant="h6"
        >
          {compact ? 'A' : 'Attricat'}
        </Typography>
        {!compact && (
          <Typography color="text.secondary" variant="body2">
            {t('app.dataManagement')}
          </Typography>
        )}
      </Box>
      <Divider />
      <List
        sx={{
          alignItems: 'center',
          display: 'flex',
          flexDirection: 'column',
          flexGrow: compact ? 0 : 1,
          overflowY: compact ? 'visible' : 'auto',
          px: compact ? 0.5 : 1,
          py: 1.5,
          width: '100%',
        }}
      >
        {primaryItems.map((item) => (
          <Tooltip
            key={item.to}
            placement="right"
            title={compact ? t(item.labelKey) : ''}
          >
            <ListItemButton
              aria-label={t(item.labelKey)}
              component={Link}
              onClick={() => {
                if (compact && item.to === '/') {
                  setCompactExplore(!compactExploreOpen);
                  return;
                }
                if (compact) {
                  setCompactManageOpen(false);
                  setCompactExploreOpen(false);
                  onCompactManageOpenChange?.(false);
                }
                onNavigate?.();
              }}
              selected={
                item.to === '/'
                  ? pathname === item.to && !search.locked
                  : pathname.startsWith(item.to)
              }
              sx={itemSx}
              to={item.to}
            >
              <ListItemIcon sx={compact ? { minWidth: 0 } : undefined}>
                {item.icon}
              </ListItemIcon>
              {compact ? (
                <Typography
                  sx={{
                    fontSize: '0.65rem',
                    lineHeight: 1.1,
                    textAlign: 'center',
                  }}
                  variant="caption"
                >
                  {t(item.labelKey)}
                </Typography>
              ) : (
                <ListItemText primary={t(item.labelKey)} />
              )}
            </ListItemButton>
          </Tooltip>
        ))}
        <Box sx={compact ? { width: '100%' } : { px: 1 }}>
          <QueryErrorNotice
            error={pinnedExplore.error}
            isRetrying={pinnedExplore.isFetching}
            onRetry={() => void pinnedExplore.refetch()}
          />
        </Box>
        {!compact &&
          pinnedExplore.data?.map((item) => (
            <Tooltip
              key={item.blueprint_code}
              placement="right"
              title={compact ? item.blueprint_name : ''}
            >
              <RouterListItemButton
                aria-label={item.blueprint_name}
                onClick={() => {
                  if (compact) setCompactManageOpen(false);
                  onNavigate?.();
                }}
                search={{ blueprint: item.blueprint_code, locked: true }}
                selected={
                  pathname === '/' && search.blueprint === item.blueprint_code
                }
                style={{ color: 'inherit', textDecoration: 'none' }}
                sx={itemSx}
                to="/"
              >
                <ListItemIcon sx={compact ? { minWidth: 0 } : undefined}>
                  <TravelExploreOutlinedIcon />
                </ListItemIcon>
                {compact ? (
                  <Typography
                    noWrap
                    sx={{ fontSize: '0.65rem', maxWidth: 72 }}
                    variant="caption"
                  >
                    {item.blueprint_name}
                  </Typography>
                ) : (
                  <ListItemText primary={item.blueprint_name} />
                )}
              </RouterListItemButton>
            </Tooltip>
          ))}
        <ExtensionOutlet outlet="navigation" />
        {compact && (
          <Tooltip placement="right" title={t('navigation.manage')}>
            <ListItemButton
              aria-expanded={isCompactManageOpen}
              aria-label={t('navigation.manage')}
              component={Link}
              onClick={() => {
                setCompactExploreOpen(false);
                setCompactManageOpen(true);
              }}
              selected={
                pathname === '/manage' || pathname.startsWith('/manage/')
              }
              sx={itemSx}
              to="/manage"
            >
              <ListItemIcon sx={{ minWidth: 0 }}>
                <SettingsOutlinedIcon />
              </ListItemIcon>
              <Typography
                sx={{
                  fontSize: '0.65rem',
                  lineHeight: 1.1,
                  textAlign: 'center',
                }}
                variant="caption"
              >
                {t('navigation.manage')}
              </Typography>
            </ListItemButton>
          </Tooltip>
        )}
        {!compact && (
          <>
            <ListItemButton
              aria-expanded={isManageOpen}
              onClick={() => setManageOpen((open) => !open)}
              sx={{ mt: 1 }}
            >
              <ListItemText
                primary={
                  <Typography color="text.secondary" variant="overline">
                    {t('navigation.manage')}
                  </Typography>
                }
              />
              {isManageOpen ? <ExpandMoreIcon /> : <ChevronRightIcon />}
            </ListItemButton>
            <Collapse in={isManageOpen} timeout="auto" unmountOnExit>
              <ListItemButton
                component={Link}
                onClick={onNavigate}
                selected={pathname === '/manage' || pathname === '/manage/'}
                to="/manage"
              >
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
                  <ListItemIcon>{item.icon}</ListItemIcon>
                  <ListItemText primary={t(item.labelKey)} />
                </ListItemButton>
              ))}
            </Collapse>
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
            left: drawerWidth,
            overflowY: 'auto',
            position: 'absolute',
            top: 0,
            width: managementSidebarWidth,
            zIndex: 1,
          }}
        >
          <Box sx={{ px: 3, py: 2 }}>
            <Typography variant="h6">{t('navigation.manage')}</Typography>
          </Box>
          <Divider />
          <List sx={{ px: 1, py: 1.5 }}>
            <ListItemButton
              component={Link}
              selected={pathname === '/manage' || pathname === '/manage/'}
              to="/manage"
            >
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
                <ListItemText primary={t(item.labelKey)} />
              </ListItemButton>
            ))}
          </List>
        </Box>
      )}
      {compact && compactExploreOpen && (
        <Box
          aria-label={t('navigation.entityExplorer')}
          component="nav"
          sx={{
            backgroundColor: 'background.paper',
            borderColor: 'divider',
            borderRight: 1,
            boxShadow: 3,
            height: '100%',
            left: drawerWidth,
            overflowY: 'auto',
            position: 'absolute',
            top: 0,
            width: managementSidebarWidth,
            zIndex: 1,
          }}
        >
          <Box sx={{ px: 3, py: 2 }}>
            <Typography variant="h6">
              {t('navigation.entityExplorer')}
            </Typography>
          </Box>
          <Divider />
          <List sx={{ px: 1, py: 1.5 }}>
            <ListItemButton
              component={Link}
              selected={pathname === '/' && !search.locked}
              to="/"
            >
              <ListItemIcon>
                <TravelExploreOutlinedIcon />
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
                  pathname === '/' && search.blueprint === item.blueprint_code
                }
                style={{ color: 'inherit', textDecoration: 'none' }}
                to="/"
              >
                <ListItemIcon>
                  <BookmarkBorderOutlinedIcon />
                </ListItemIcon>
                <ListItemText primary={item.blueprint_name} />
              </RouterListItemButton>
            ))}
          </List>
        </Box>
      )}
      {compact && <Box sx={{ flexGrow: 1 }} />}
      <Divider />
      <List
        sx={{
          alignItems: 'center',
          display: 'flex',
          flexDirection: 'column',
          px: compact ? 0.5 : 1,
          py: 1.5,
        }}
      >
        {visibleNavigationItems
          .filter((item) => item.to === '/profile')
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
                  if (compact) {
                    setCompactManageOpen(false);
                    setCompactExploreOpen(false);
                    onCompactManageOpenChange?.(false);
                  }
                  onNavigate?.();
                }}
                selected={pathname.startsWith(item.to)}
                sx={itemSx}
                to={item.to}
              >
                <ListItemIcon sx={compact ? { minWidth: 0 } : undefined}>
                  {item.icon}
                </ListItemIcon>
                {compact ? (
                  <Typography
                    sx={{
                      fontSize: '0.65rem',
                      lineHeight: 1.1,
                      textAlign: 'center',
                    }}
                    variant="caption"
                  >
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
              <Typography
                sx={{
                  fontSize: '0.65rem',
                  lineHeight: 1.1,
                  textAlign: 'center',
                }}
                variant="caption"
              >
                {t('navigation.signOut')}
              </Typography>
            ) : (
              <ListItemText primary={t('navigation.signOut')} />
            )}
          </ListItemButton>
        </Tooltip>
      </List>
    </Box>
  );
};
