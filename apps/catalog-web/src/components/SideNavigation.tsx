import AssessmentOutlinedIcon from '@mui/icons-material/AssessmentOutlined';
import SmartToyOutlinedIcon from '@mui/icons-material/SmartToyOutlined';
import CategoryOutlinedIcon from '@mui/icons-material/CategoryOutlined';
import FolderOutlinedIcon from '@mui/icons-material/FolderOutlined';
import TravelExploreOutlinedIcon from '@mui/icons-material/TravelExploreOutlined';
import ManageAccountsOutlinedIcon from '@mui/icons-material/ManageAccountsOutlined';
import PersonOutlinedIcon from '@mui/icons-material/PersonOutlined';
import { useQuery } from '@tanstack/react-query';
import { Link, useRouterState } from '@tanstack/react-router';
import { currentSession } from '../features/auth/api';
import {
  Box,
  Divider,
  List,
  ListItemButton,
  ListItemIcon,
  ListItemText,
  Typography,
} from '@mui/material';

const navigationItems = [
  { icon: <TravelExploreOutlinedIcon />, label: 'Entity explorer', to: '/' },
  { icon: <CategoryOutlinedIcon />, label: 'Blueprints', to: '/blueprints' },
  { icon: <FolderOutlinedIcon />, label: 'Contexts', to: '/contexts' },
  { icon: <SmartToyOutlinedIcon />, label: 'Agents', to: '/agents' },
  {
    icon: <AssessmentOutlinedIcon />,
    label: 'Data health',
    to: '/data-health',
  },
  { icon: <PersonOutlinedIcon />, label: 'Profile', to: '/profile' },
  {
    icon: <ManageAccountsOutlinedIcon />,
    label: 'Workspace management',
    to: '/workspace/members',
  },
] as const;

export const drawerWidth = 264;

export const SideNavigation = ({ onNavigate }: { onNavigate?: () => void }) => {
  const pathname = useRouterState({
    select: (state) => state.location.pathname,
  });
  const session = useQuery({
    queryKey: ['auth', 'session'],
    queryFn: currentSession,
  });
  const canManageWorkspace = Boolean(
    session.data?.capabilities?.members_manage ||
    session.data?.capabilities?.roles_manage ||
    session.data?.capabilities?.tokens_manage,
  );

  return (
    <Box sx={{ width: drawerWidth }}>
      <Box sx={{ px: 3, py: 2 }}>
        <Typography color="primary" sx={{ fontWeight: 700 }} variant="h6">
          Catalog
        </Typography>
        <Typography color="text.secondary" variant="body2">
          Data management
        </Typography>
      </Box>
      <Divider />
      <List sx={{ px: 1, py: 1.5 }}>
        {navigationItems
          .filter(
            (item) => item.to !== '/workspace/members' || canManageWorkspace,
          )
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
      </List>
    </Box>
  );
};
