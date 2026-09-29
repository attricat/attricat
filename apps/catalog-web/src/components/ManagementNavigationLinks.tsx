import { Link } from '@tanstack/react-router';
import { ListItemButton, ListItemIcon, ListItemText } from '@mui/material';
import { createElement } from 'react';
import { useTranslation } from 'react-i18next';
import {
  getVisibleManagementNavigationItems,
  navigationRoutes,
} from './navigation';
import { ManagementIcon } from './systemIcons';

type ManagementNavigationLinksProps = {
  items: ReturnType<typeof getVisibleManagementNavigationItems>;
  onNavigate?: () => void;
  pathname: string;
};

export const ManagementNavigationLinks = ({
  items,
  onNavigate,
  pathname,
}: ManagementNavigationLinksProps) => {
  const { t } = useTranslation();
  return (
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
      {items.map((item) => (
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
  );
};
