import { useQuery } from '@tanstack/react-query';
import {
  Alert,
  Avatar,
  Box,
  Divider,
  Skeleton,
  Stack,
  Typography,
} from '@mui/material';
import { GlobeIcon, LanguagesIcon } from 'lucide-react';
import type { ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { monoFontFamily } from '../../app/theme';
import { LanguageSwitcher } from '../../components/LanguageSwitcher';
import { ProfileIcon } from '../../components/systemIcons';
import { currentSession } from '../auth/api';
import { authQueryKeys } from '../auth/queryKeys';
import { ProfileSection } from './ProfileSection';
import { TimeZonePreference } from './TimeZonePreference';
import {
  profileAvatarSize,
  profileSkeletonWidth,
  profileValueSkeletonWidth,
} from './constants';

const initials = (name: string) =>
  name
    .split(/[\s@._-]+/)
    .filter(Boolean)
    .slice(0, 2)
    .map((part) => part[0]?.toUpperCase())
    .join('');

const AccountDetail = ({
  label,
  value,
}: {
  label: string;
  value: ReactNode;
}) => (
  <>
    <Typography color="text.secondary" component="dt" variant="subtitle1">
      {label}
    </Typography>
    <Typography
      component="dd"
      sx={{ fontFamily: monoFontFamily, m: 0, overflowWrap: 'anywhere' }}
      variant="body2"
    >
      {value ?? <Skeleton width={profileValueSkeletonWidth} />}
    </Typography>
  </>
);

export const ProfilePage = () => {
  const { t } = useTranslation();
  const session = useQuery({
    queryKey: authQueryKeys.session(),
    queryFn: currentSession,
  });
  const account = session.data;
  const name = account?.display_name ?? account?.email;
  return (
    <Stack spacing={6}>
      {session.isError && (
        <Alert severity="error">{session.error.message}</Alert>
      )}
      <ProfileSection
        description={t('profile.accountDescription')}
        icon={ProfileIcon}
        title={t('profile.accountDetails')}
      >
        <Stack direction="row" spacing={4} sx={{ alignItems: 'center' }}>
          <Avatar
            sx={{
              bgcolor: 'action.selected',
              color: 'primary.main',
              fontWeight: 600,
              height: profileAvatarSize,
              width: profileAvatarSize,
            }}
          >
            {name ? initials(name) : undefined}
          </Avatar>
          <Box sx={{ minWidth: 0 }}>
            <Typography component="p" noWrap variant="h4">
              {name ?? <Skeleton width={profileSkeletonWidth} />}
            </Typography>
            <Typography color="text.secondary" noWrap variant="body2">
              {account
                ? account.display_name
                  ? account.email
                  : t('profile.displayNameNotSet')
                : null}
            </Typography>
          </Box>
        </Stack>
        <Divider sx={{ my: 5 }} />
        <Box
          component="dl"
          sx={{
            columnGap: 8,
            display: 'grid',
            gridTemplateColumns: { xs: '1fr', sm: 'max-content 1fr' },
            m: 0,
            rowGap: { xs: 1, sm: 3 },
            '& dd:not(:last-of-type)': { mb: { xs: 3, sm: 0 } },
          }}
        >
          <AccountDetail label={t('profile.userId')} value={account?.user_id} />
          <AccountDetail
            label={t('profile.activeWorkspace')}
            value={account?.workspace_id}
          />
        </Box>
      </ProfileSection>
      <ProfileSection
        description={t('profile.languageDescription')}
        icon={LanguagesIcon}
        title={t('language.label')}
      >
        <LanguageSwitcher />
      </ProfileSection>
      <ProfileSection
        description={t('timeZone.description')}
        icon={GlobeIcon}
        title={t('timeZone.label')}
      >
        <TimeZonePreference disabled={!account} />
      </ProfileSection>
    </Stack>
  );
};
