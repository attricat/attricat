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
import { useQuery } from '@tanstack/react-query';
import { Link, useRouterState } from '@tanstack/react-router';
import { useState } from 'react';
import { currentSession } from '../features/auth/api';
import { ExtensionOutlet } from '../features/extensions/ExtensionOutlet';
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
  { icon: <TravelExploreOutlinedIcon />, label: 'Entity explorer', to: '/' },
  {
    icon: <CategoryOutlinedIcon />,
    label: 'Blueprints',
    to: '/manage/blueprints',
  },
  { icon: <FolderOutlinedIcon />, label: 'Contexts', to: '/manage/contexts' },
  { icon: <SmartToyOutlinedIcon />, label: 'Agents', to: '/agents' },
  {
    icon: <AssessmentOutlinedIcon />,
    label: 'Data health',
    to: '/manage/data-health',
  },
  {
    icon: <FactCheckOutlinedIcon />,
    label: 'Activity / Audit log',
    to: '/manage/audit-log',
  },
  { icon: <PersonOutlinedIcon />, label: 'Profile', to: '/profile' },
  {
    icon: <ManageAccountsOutlinedIcon />,
    label: 'Workspace management',
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
  const pathname = useRouterState({
    select: (state) => state.location.pathname,
  });
  const [manageOpen, setManageOpen] = useState(false);
  const isManageOpen = manageOpen || pathname.startsWith('/manage/');
  const session = useQuery({
    queryKey: ['auth', 'session'],
    queryFn: currentSession,
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
        session.data?.capabilities?.audit_read),
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
          Catalog
        </Typography>
        <Typography color="text.secondary" variant="body2">
          Data management
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
              <ListItemText primary={item.label} />
            </ListItemButton>
          ))}
        <ExtensionOutlet outlet="navigation" />
        <ListItemButton
          aria-expanded={isManageOpen}
          onClick={() => setManageOpen((open) => !open)}
          sx={{ mt: 1 }}
        >
          <ListItemText
            primary="Manage"
            primaryTypographyProps={{
              color: 'text.secondary',
              variant: 'overline',
            }}
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
                <ListItemText primary={item.label} />
              </ListItemButton>
            ))}
        </Collapse>
      </List>
      <Divider />
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
              <ListItemText primary={item.label} />
            </ListItemButton>
          ))}
        <ListItemButton onClick={onSignOut}>
          <ListItemIcon>
            <LogoutOutlinedIcon />
          </ListItemIcon>
          <ListItemText primary="Sign out" />
        </ListItemButton>
      </List>
    </Box>
  );
};
