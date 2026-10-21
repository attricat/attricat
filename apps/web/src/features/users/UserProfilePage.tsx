import { useQuery } from '@tanstack/react-query';
import {
  Alert,
  Box,
  Chip,
  Link,
  Paper,
  Skeleton,
  Stack,
  Typography,
} from '@mui/material';
import { PencilIcon } from 'lucide-react';
import type { ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { ApiRequestError } from '../../api/request';
import { compactIconSize } from '../../components/iconSizes';
import { EmptyState } from '../../components/EmptyState';
import { PageContainer } from '../../components/PageContainer';
import { PageHeader } from '../../components/PageHeader';
import { QueryErrorNotice } from '../../components/QueryErrorNotice';
import { RouterButton } from '../../components/RouterLink';
import { ProfileIcon, TeamIcon } from '../../components/systemIcons';
import { UserAvatar } from '../../components/UserAvatar';
import { Timestamp } from '../../time/Timestamp';
import { currentSession } from '../auth/api';
import { authQueryKeys } from '../auth/queryKeys';
import { getUserProfile } from './api';
import { userProfileAvatarSize, userProfileSkeletonWidth } from './constants';
import { userQueryKeys } from './queryKeys';

const NOT_FOUND = 404;

const ProfileDetail = ({
  label,
  children,
}: {
  label: string;
  children: ReactNode;
}) => (
  <>
    <Typography color="text.secondary" component="dt" variant="subtitle2">
      {label}
    </Typography>
    <Typography component="dd" sx={{ m: 0, overflowWrap: 'anywhere' }}>
      {children}
    </Typography>
  </>
);

/** A workspace member as every catalog reader sees them. */
export const UserProfilePage = ({ userId }: { userId: string }) => {
  const { t } = useTranslation();
  const profile = useQuery({
    queryKey: userQueryKeys.profile(userId),
    queryFn: ({ signal }) => getUserProfile(userId, signal),
    retry: (count, error) =>
      !(error instanceof ApiRequestError && error.status === NOT_FOUND) &&
      count < 3,
  });
  const session = useQuery({
    queryKey: authQueryKeys.session(),
    queryFn: currentSession,
  });
  const user = profile.data;
  const name = user ? user.display_name?.trim() || user.email : undefined;
  const isSelf = session.data?.user_id === userId;
  const notFound =
    profile.error instanceof ApiRequestError &&
    profile.error.status === NOT_FOUND;

  return (
    <PageContainer maxWidth="md">
      <Stack spacing={4}>
        <PageHeader
          actions={
            isSelf && (
              <RouterButton
                startIcon={<PencilIcon />}
                to="/profile"
                variant="outlined"
              >
                {t('users.editOwnProfile')}
              </RouterButton>
            )
          }
          icon={ProfileIcon}
          title={
            name ??
            (notFound ? (
              t('users.notFoundTitle')
            ) : (
              <Skeleton width={userProfileSkeletonWidth} />
            ))
          }
        />
        {notFound ? (
          <Alert severity="info">{t('users.notFound')}</Alert>
        ) : (
          <QueryErrorNotice
            error={profile.error}
            isRetrying={profile.isFetching}
            onRetry={() => void profile.refetch()}
          />
        )}
        {user && (
          <>
            <Paper sx={{ p: 4 }}>
              <Stack
                direction={{ xs: 'column', sm: 'row' }}
                spacing={4}
                sx={{ alignItems: { sm: 'flex-start' } }}
              >
                <UserAvatar
                  avatarFileId={user.avatar_file_id}
                  name={name}
                  size={userProfileAvatarSize}
                  sx={{ flexShrink: 0 }}
                />
                <Box
                  component="dl"
                  sx={{
                    columnGap: 4,
                    display: 'grid',
                    flex: 1,
                    gridTemplateColumns: { sm: 'max-content 1fr' },
                    m: 0,
                    minWidth: 0,
                    rowGap: 1,
                  }}
                >
                  <ProfileDetail label={t('users.email')}>
                    <Link href={`mailto:${user.email}`}>{user.email}</Link>
                  </ProfileDetail>
                  <ProfileDetail label={t('users.membership')}>
                    {user.active ? (
                      t('users.activeMember')
                    ) : (
                      <Chip
                        label={t('principals.inactiveUser')}
                        size="small"
                        variant="outlined"
                      />
                    )}
                  </ProfileDetail>
                  <ProfileDetail label={t('users.memberSince')}>
                    <Timestamp style="date" value={user.member_since} />
                  </ProfileDetail>
                </Box>
              </Stack>
            </Paper>
            <Box component="section" aria-labelledby="user-profile-teams">
              <Typography
                component="h2"
                id="user-profile-teams"
                sx={{ mb: 2 }}
                variant="h6"
              >
                {t('principals.teams')}
              </Typography>
              <Paper>
                {user.teams.length ? (
                  <Stack
                    direction="row"
                    sx={{ flexWrap: 'wrap', gap: 1, p: 2 }}
                  >
                    {user.teams.map((team) => (
                      <Chip
                        icon={<TeamIcon aria-hidden size={compactIconSize} />}
                        key={team.id}
                        label={team.name}
                        title={team.code}
                        variant="outlined"
                      />
                    ))}
                  </Stack>
                ) : (
                  <EmptyState icon={TeamIcon} title={t('users.noTeams')} />
                )}
              </Paper>
            </Box>
          </>
        )}
      </Stack>
    </PageContainer>
  );
};
