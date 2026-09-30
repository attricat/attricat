import { Link, type ToOptions } from '@tanstack/react-router';
import { Box, Tab, Tabs } from '@mui/material';
import type { LucideIcon } from 'lucide-react';
import type { ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { SettingsPage } from '../../components/CenteredPage';
import { PageHeader } from '../../components/PageHeader';
import { compactIconSize } from '../../components/iconSizes';
import { PersonalTokenIcon, ProfileIcon } from '../../components/systemIcons';
import { profileTabIds } from './constants';

const profileTabs = [
  { icon: ProfileIcon, label: 'profile.account', to: '/profile' },
  {
    icon: PersonalTokenIcon,
    label: 'profile.tokenList',
    to: '/profile/personal-access-tokens',
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
  return (
    <SettingsPage>
      <PageHeader
        description={t('profile.description')}
        icon={ProfileIcon}
        title={t('profile.title')}
      />
      <Tabs aria-label={t('profile.title')} sx={{ mb: 6, mt: 5 }} value={tab}>
        {profileTabs.map((item, index) => (
          <Tab
            aria-controls={profileTabIds.panel(index)}
            component={Link}
            icon={<item.icon aria-hidden size={compactIconSize} />}
            iconPosition="start"
            id={profileTabIds.tab(index)}
            key={item.to}
            label={t(item.label)}
            to={item.to}
            sx={{ '& .MuiTab-icon': { mr: 2 } }}
            value={index}
          />
        ))}
      </Tabs>
      <Box
        aria-labelledby={profileTabIds.tab(tab)}
        id={profileTabIds.panel(tab)}
        role="tabpanel"
      >
        {children}
      </Box>
    </SettingsPage>
  );
};
