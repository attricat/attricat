import AssessmentOutlinedIcon from '@mui/icons-material/AssessmentOutlined';
import CategoryOutlinedIcon from '@mui/icons-material/CategoryOutlined';
import FolderOutlinedIcon from '@mui/icons-material/FolderOutlined';
import TravelExploreOutlinedIcon from '@mui/icons-material/TravelExploreOutlined';
import { Link, useRouterState } from '@tanstack/react-router';
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
  {
    icon: <AssessmentOutlinedIcon />,
    label: 'Data health',
    to: '/data-health',
  },
] as const;

export const drawerWidth = 264;

export const SideNavigation = ({ onNavigate }: { onNavigate?: () => void }) => {
  const pathname = useRouterState({
    select: (state) => state.location.pathname,
  });

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
        {navigationItems.map((item) => (
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
