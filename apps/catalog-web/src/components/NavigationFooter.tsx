import { Divider, List, useTheme } from '@mui/material';
import { LogOutIcon, MoonIcon, SunIcon } from 'lucide-react';
import { createElement } from 'react';
import { useTranslation } from 'react-i18next';
import { useColorMode } from '../app/colorMode';
import { profileNavigationItem } from './navigation';
import { NavigationItem } from './NavigationItem';
import { DocumentationIcon } from './systemIcons';

type NavigationFooterProps = {
  compact: boolean;
  documentationHref: string;
  onDocumentationClick?: () => void;
  onProfileClick: () => void;
  onSignOut?: () => void;
  pathname: string;
};

export const NavigationFooter = ({
  compact,
  documentationHref,
  onDocumentationClick,
  onProfileClick,
  onSignOut,
  pathname,
}: NavigationFooterProps) => {
  const { t } = useTranslation();
  const darkMode = useTheme().palette.mode === 'dark';
  const setColorMode = useColorMode((state) => state.setPreference);
  const colorModeLabel = t(
    darkMode ? 'navigation.lightMode' : 'navigation.darkMode',
  );

  return (
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
        <NavigationItem
          compact={compact}
          href={documentationHref}
          icon={<DocumentationIcon />}
          label={t('navigation.documentation')}
          onClick={onDocumentationClick}
        />
        <NavigationItem
          compact={compact}
          icon={createElement(profileNavigationItem.icon)}
          label={t(profileNavigationItem.labelKey)}
          onClick={onProfileClick}
          selected={pathname.startsWith(profileNavigationItem.to)}
          to={profileNavigationItem.to}
        />
        <NavigationItem
          compact={compact}
          icon={darkMode ? <SunIcon /> : <MoonIcon />}
          label={colorModeLabel}
          onClick={() => setColorMode(darkMode ? 'light' : 'dark')}
        />
        <NavigationItem
          compact={compact}
          icon={<LogOutIcon />}
          label={t('navigation.signOut')}
          onClick={onSignOut}
        />
      </List>
    </>
  );
};
