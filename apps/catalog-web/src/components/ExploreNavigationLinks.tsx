import { Link } from '@tanstack/react-router';
import {
  ListItemButton,
  ListItemIcon,
  ListItemText,
  ListSubheader,
} from '@mui/material';
import { useTranslation } from 'react-i18next';
import type { listSidebarExploreNavigation } from '../features/workspace/api';
import { navigationRoutes } from './navigation';
import { NavigationIcon } from './NavigationIcon';
import { RouterListItemButton } from './RouterLink';
import { ExplorerIcon, ExplorerShortcutIcon } from './systemIcons';
import { lexiconText } from '../features/lexicon/lexicon';

type ExploreShortcut = Awaited<
  ReturnType<typeof listSidebarExploreNavigation>
>[number];

type ExploreNavigationLinksProps = {
  allRecordsSelected: boolean;
  onNavigate?: () => void;
  selectedBlueprintCode?: string;
  shortcuts: readonly ExploreShortcut[] | undefined;
};

export const ExploreNavigationLinks = ({
  allRecordsSelected,
  onNavigate,
  selectedBlueprintCode,
  shortcuts,
}: ExploreNavigationLinksProps) => {
  const { t } = useTranslation();
  return (
    <>
      <ListItemButton
        component={Link}
        onClick={onNavigate}
        selected={allRecordsSelected}
        to={navigationRoutes.explore}
      >
        <ListItemIcon>
          <NavigationIcon icon={ExplorerIcon} />
        </ListItemIcon>
        <ListItemText primary={t('navigation.allRecords')} />
      </ListItemButton>
      {shortcuts?.length ? (
        <ListSubheader disableSticky>{t('navigation.shortcuts')}</ListSubheader>
      ) : null}
      {shortcuts?.map((item) => (
        <RouterListItemButton
          aria-label={lexiconText(item.blueprint_name)}
          key={item.blueprint_code}
          onClick={onNavigate}
          search={{ blueprint: item.blueprint_code, locked: true }}
          selected={selectedBlueprintCode === item.blueprint_code}
          style={{ color: 'inherit', textDecoration: 'none' }}
          to={navigationRoutes.explore}
        >
          <ListItemIcon>
            <NavigationIcon icon={ExplorerShortcutIcon} />
          </ListItemIcon>
          <ListItemText primary={lexiconText(item.blueprint_name)} />
        </RouterListItemButton>
      ))}
    </>
  );
};
