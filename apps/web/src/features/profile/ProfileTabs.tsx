import type { ToOptions } from '@tanstack/react-router';
import { Box, Tabs } from '@mui/material';
import type { LucideIcon } from 'lucide-react';
import type { ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { SettingsPage } from '../../components/CenteredPage';
import { PageHeader } from '../../components/PageHeader';
import { compactIconSize } from '../../components/iconSizes';
import {
  ExtensionRunIcon,
  PersonalTokenIcon,
  ProfileIcon,
} from '../../components/systemIcons';
import { RouterTab } from '../../components/RouterLink';
import { useTabAccessibility } from '../../components/useTabAccessibility';

const profileTabs = [
  { icon: ProfileIcon, label: 'profile.account', to: '/profile' },
  {
    icon: PersonalTokenIcon,
    label: 'profile.tokenList',
    to: '/profile/personal-access-tokens',
  },
  {
    icon: ExtensionRunIcon,
    label: 'profile.extensionRuns',
    to: '/profile/extension-runs',
  },
] as const satisfies ReadonlyArray<{
  icon: LucideIcon;
  label: string;
  to: ToOptions['to'];
}>;

type ProfileTabsProps = {
  children: ReactNode;
  tab: number;
};

export const ProfileTabs = ({ children, tab }: ProfileTabsProps) => {
  const { t } = useTranslation();
  const tabId = useTabAccessibility();
  return (
    <SettingsPage>
      <PageHeader
        description={t('profile.description')}
        icon={ProfileIcon}
        title={t('profile.title')}
      />
      <Tabs aria-label={t('profile.title')} sx={{ mb: 6, mt: 5 }} value={tab}>
        {profileTabs.map((item, index) => (
          <RouterTab
            {...tabId.tab(index)}
            icon={<item.icon aria-hidden size={compactIconSize} />}
            key={item.to}
            label={t(item.label)}
            to={item.to}
            value={index}
          />
        ))}
      </Tabs>
      <Box {...tabId.panel(tab)}>{children}</Box>
    </SettingsPage>
  );
};
