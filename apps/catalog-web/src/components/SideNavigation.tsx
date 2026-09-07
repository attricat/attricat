import AssessmentOutlinedIcon from '@mui/icons-material/AssessmentOutlined';
import ChevronRightIcon from '@mui/icons-material/ChevronRight';
import ExpandMoreIcon from '@mui/icons-material/ExpandMore';
import FactCheckOutlinedIcon from '@mui/icons-material/FactCheckOutlined';
import SmartToyOutlinedIcon from '@mui/icons-material/SmartToyOutlined';
import CategoryOutlinedIcon from '@mui/icons-material/CategoryOutlined';
import FolderOutlinedIcon from '@mui/icons-material/FolderOutlined';
import TravelExploreOutlinedIcon from '@mui/icons-material/TravelExploreOutlined';
import ManageAccountsOutlinedIcon from '@mui/icons-material/ManageAccountsOutlined';
import PersonOutlinedIcon from '@mui/icons-material/PersonOutlined';
import LogoutOutlinedIcon from '@mui/icons-material/LogoutOutlined';
import BoltOutlinedIcon from '@mui/icons-material/BoltOutlined';
import { useQuery } from '@tanstack/react-query';
import { Link, useRouterState } from '@tanstack/react-router';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { currentSession } from '../features/auth/api';
import { listSidebarExploreNavigation } from '../features/workspace/api';
import { workspaceQueryKeys } from '../features/workspace/query-keys';
import { ExtensionOutlet } from '../features/extensions/ExtensionOutlet';
import { LanguageSwitcher } from './LanguageSwitcher';
import {
  Box,
  Collapse,
  Divider,
  List,
  ListItemButton,
  ListItemIcon,
  ListItemText,
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

export const drawerWidth = 264;

export const SideNavigation = ({
  onNavigate,
  onSignOut,
}: {
  onNavigate?: () => void;
  onSignOut?: () => void;
}) => {
  const { t } = useTranslation();
  const { pathname, search } = useRouterState({
    select: (state) => state.location,
  });
  const [manageOpen, setManageOpen] = useState(false);
  const isManageOpen = manageOpen || pathname.startsWith('/manage/');
  const session = useQuery({
    queryKey: ['auth', 'session'],
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

  return (
    <Box
      sx={{
        display: 'flex',
        flexDirection: 'column',
        height: '100%',
        width: drawerWidth,
      }}
    >
      <Box sx={{ px: 3, py: 2 }}>
        <Typography color="primary" sx={{ fontWeight: 700 }} variant="h6">
          Attricat
        </Typography>
        <Typography color="text.secondary" variant="body2">
          {t('app.dataManagement')}
        </Typography>
      </Box>
      <Divider />
      <List sx={{ flexGrow: 1, overflowY: 'auto', px: 1, py: 1.5 }}>
        {visibleNavigationItems
          .filter((item) => item.to === '/' || item.to === '/agents')
          .map((item) => (
            <ListItemButton
              component={Link}
              key={item.to}
              onClick={onNavigate}
              selected={
                item.to === '/'
                  ? pathname === item.to
                  : pathname.startsWith(item.to)
              }
              to={item.to}
            >
              <ListItemIcon>{item.icon}</ListItemIcon>
              <ListItemText primary={t(item.labelKey)} />
            </ListItemButton>
          ))}
        {pinnedExplore.data?.map((item) => (
          <Link
            key={item.blueprint_code}
            onClick={onNavigate}
            search={{ blueprint: item.blueprint_code, locked: true }}
            style={{ color: 'inherit', textDecoration: 'none' }}
            to="/"
          >
            <ListItemButton
              selected={
                pathname === '/' && search.blueprint === item.blueprint_code
              }
            >
              <ListItemIcon>
                <TravelExploreOutlinedIcon />
              </ListItemIcon>
              <ListItemText primary={item.blueprint_name} />
            </ListItemButton>
          </Link>
        ))}
        <ExtensionOutlet outlet="navigation" />
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
          {visibleNavigationItems
            .filter(
              (item) =>
                item.to !== '/' &&
                item.to !== '/agents' &&
                item.to !== '/profile',
            )
            .map((item) => (
              <ListItemButton
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
      </List>
      <Divider />
      <Box sx={{ px: 2, py: 1.5 }}>
        <LanguageSwitcher />
      </Box>
      <List sx={{ px: 1, py: 1.5 }}>
        {visibleNavigationItems
          .filter((item) => item.to === '/profile')
          .map((item) => (
            <ListItemButton
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
        <ListItemButton onClick={onSignOut}>
          <ListItemIcon>
            <LogoutOutlinedIcon />
          </ListItemIcon>
          <ListItemText primary={t('navigation.signOut')} />
        </ListItemButton>
      </List>
    </Box>
  );
};
