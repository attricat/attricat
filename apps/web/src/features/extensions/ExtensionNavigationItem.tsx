import { ListItemIcon, ListItemText } from '@mui/material';
import { useRouterState } from '@tanstack/react-router';
import { NavigationIcon } from '../../components/NavigationIcon';
import { RouterListItemButton } from '../../components/RouterLink';
import { ExtensionIcon } from '../../components/systemIcons';
import type { ExtensionContribution } from './api';

type ExtensionNavigationItemProps = {
  contribution: ExtensionContribution;
  onNavigate?: () => void;
};

export const ExtensionNavigationItem = ({
  contribution,
  onNavigate,
}: ExtensionNavigationItemProps) => {
  const { pathname } = useRouterState({ select: (state) => state.location });
  if (contribution.kind !== 'navigation' || !contribution.route) return null;
  const target = `/extensions/${contribution.extension_id}/${contribution.route}`;
  const label = contribution.title ?? contribution.id;
  return (
    <RouterListItemButton
      aria-label={label}
      onClick={onNavigate}
      selected={pathname === target}
      to="/extensions/$extensionId/$contributionId"
      params={{
        extensionId: contribution.extension_id,
        contributionId: contribution.route,
      }}
    >
      <ListItemIcon>
        <NavigationIcon icon={ExtensionIcon} />
      </ListItemIcon>
      <ListItemText primary={label} />
    </RouterListItemButton>
  );
};
