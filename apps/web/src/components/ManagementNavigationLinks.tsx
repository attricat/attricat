import { isWithinRoute } from './sideNavigationLayout';
import { Fragment } from 'react';
import { Link } from '@tanstack/react-router';
import {
  ListItemButton,
  ListItemIcon,
  ListItemText,
  ListSubheader,
} from '@mui/material';
import { useTranslation } from 'react-i18next';
import {
  getVisibleManagementNavigationItems,
  navigationRoutes,
} from './navigation';
import { NavigationIcon } from './NavigationIcon';
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
          <NavigationIcon icon={ManagementIcon} />
        </ListItemIcon>
        <ListItemText primary={t('navigation.dashboard')} />
      </ListItemButton>
      {items.map((item, index) => (
        <Fragment key={item.to}>
          {item.group !== items[index - 1]?.group && (
            <ListSubheader component="div" disableSticky>
              {t(`navigation.managementGroups.${item.group}`)}
            </ListSubheader>
          )}
          <ListItemButton
            aria-label={t(item.labelKey)}
            component={Link}
            onClick={onNavigate}
            selected={isWithinRoute(pathname, item.to)}
            to={item.to}
          >
            <ListItemIcon>
              <NavigationIcon icon={item.icon} />
            </ListItemIcon>
            <ListItemText primary={t(item.labelKey)} />
          </ListItemButton>
        </Fragment>
      ))}
    </>
  );
};
