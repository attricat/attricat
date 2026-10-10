import { useQuery } from '@tanstack/react-query';
import {
  Alert,
  Box,
  Button,
  CircularProgress,
  Divider,
  Skeleton,
  Stack,
  Typography,
} from '@mui/material';
import { GlobeIcon, LanguagesIcon, PencilIcon } from 'lucide-react';
import { type ReactNode, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { monoFontFamily } from '../../app/theme';
import { LanguageSwitcher } from '../../components/LanguageSwitcher';
import { ProfileIcon } from '../../components/systemIcons';
import { UserAvatar } from '../../components/UserAvatar';
import { currentSession } from '../auth/api';
import { authQueryKeys } from '../auth/queryKeys';
import { fileStatuses, THUMBNAIL_POLLING_STATUSES } from '../files/constants';
import { AvatarControls } from './AvatarControls';
import { ChangeDisplayNameDialog } from './ChangeDisplayNameDialog';
import { ProfileSection } from './ProfileSection';
import { TimeZonePreference } from './TimeZonePreference';
import {
  avatarPollMilliseconds,
  profileAvatarSize,
  profileSkeletonWidth,
  profileValueSkeletonWidth,
} from './constants';

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
    // Follow an uploaded avatar until the file worker has processed it.
    refetchInterval: (query) =>
      THUMBNAIL_POLLING_STATUSES.has(query.state.data?.avatar?.status ?? '')
        ? avatarPollMilliseconds
        : false,
  });
  const account = session.data;
  const name = account?.display_name ?? account?.email;
  const [editingName, setEditingName] = useState(false);
  const [uploadProgress, setUploadProgress] = useState<number | null>(null);
  const avatar = account?.avatar;
  const avatarProcessing =
    uploadProgress !== null ||
    THUMBNAIL_POLLING_STATUSES.has(avatar?.status ?? '');
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
        <Stack
          direction={{ xs: 'column', sm: 'row' }}
          spacing={4}
          sx={{ alignItems: { xs: 'flex-start', sm: 'center' } }}
        >
          <Stack
            direction="row"
            spacing={4}
            sx={{ alignItems: 'center', flex: 1, minWidth: 0, width: '100%' }}
          >
            <Box sx={{ flexShrink: 0, position: 'relative' }}>
              <UserAvatar
                avatarFileId={
                  avatar?.status === fileStatuses.ready ? avatar.file_id : null
                }
                name={name}
                size={profileAvatarSize}
              />
              {avatarProcessing && (
                <CircularProgress
                  aria-label={t('profile.avatarProcessing')}
                  size={profileAvatarSize}
                  sx={{ left: 0, position: 'absolute', top: 0 }}
                  thickness={2}
                  value={uploadProgress ?? undefined}
                  variant={
                    uploadProgress === null ? 'indeterminate' : 'determinate'
                  }
                />
              )}
            </Box>
            <Box sx={{ flex: 1, minWidth: 0 }}>
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
          <Button
            disabled={!account}
            onClick={() => setEditingName(true)}
            startIcon={<PencilIcon />}
            variant="outlined"
          >
            {t('profile.changeDisplayName')}
          </Button>
        </Stack>
        {avatar?.status === fileStatuses.failed && (
          <Alert severity="error" sx={{ mt: 4 }}>
            {t('profile.avatarFailed')}
          </Alert>
        )}
        <Box sx={{ mt: 4 }}>
          <AvatarControls
            disabled={!account || avatarProcessing}
            hasAvatar={Boolean(avatar)}
            onProgress={setUploadProgress}
          />
        </Box>
        {editingName && account && (
          <ChangeDisplayNameDialog
            initialName={account.display_name ?? ''}
            onClose={() => setEditingName(false)}
          />
        )}
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
