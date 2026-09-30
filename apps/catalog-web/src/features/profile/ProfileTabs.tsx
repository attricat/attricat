import { Link, type ToOptions } from '@tanstack/react-router';
import { Box, Tab, Tabs, Typography } from '@mui/material';
import type { ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { SettingsPage } from '../../components/CenteredPage';
import { profileTabIds } from './constants';

const profileTabs = [
  { label: 'profile.account', to: '/profile' },
  { label: 'profile.tokenList', to: '/profile/personal-access-tokens' },
] as const satisfies ReadonlyArray<{ label: string; to: ToOptions['to'] }>;

type ProfileTabsProps = {
  children: ReactNode;
  tab: number;
};

export const ProfileTabs = ({ children, tab }: ProfileTabsProps) => {
  const { t } = useTranslation();
  return (
    <SettingsPage>
      <Typography variant="h4">{t('profile.title')}</Typography>
      <Tabs aria-label={t('profile.title')} sx={{ my: 3 }} value={tab}>
        {profileTabs.map((item, index) => (
          <Tab
            aria-controls={profileTabIds.panel(index)}
            component={Link}
            id={profileTabIds.tab(index)}
            key={item.to}
            label={t(item.label)}
            to={item.to}
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
