import { useResourcePageTitle } from '../../app/useResourcePageTitle';
import { personName } from '../../components/personName';
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
import { useId } from 'react';
import { useTranslation } from 'react-i18next';
import { httpStatus } from '../../api/httpStatus';
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
import { getUserProfile } from './api';
import { userProfileAvatarSize, userProfileSkeletonWidth } from './constants';
import { userQueryKeys } from './queryKeys';
import { sessionOptions } from '../auth/queryOptions';
import { Detail, DetailList } from '../../components/DetailList';

/** A workspace member as every catalog reader sees them. */
export const UserProfilePage = ({ userId }: { userId: string }) => {
  const { t } = useTranslation();
  const teamsHeadingId = useId();
  const profile = useQuery({
    queryKey: userQueryKeys.profile(userId),
    queryFn: ({ signal }) => getUserProfile(userId, signal),
  });
  const session = useQuery(sessionOptions());
  const user = profile.data;
  const name = user ? personName(user.display_name, user.email) : undefined;
  useResourcePageTitle(name, t('users.userProfile'));
  const isSelf = session.data?.user_id === userId;
  const notFound =
    profile.error instanceof ApiRequestError &&
    profile.error.status === httpStatus.notFound;

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
                <DetailList sx={{ flex: 1 }}>
                  <Detail label={t('users.email')}>
                    <Link href={`mailto:${user.email}`}>{user.email}</Link>
                  </Detail>
                  <Detail label={t('users.membership')}>
                    {user.active ? (
                      t('users.activeMember')
                    ) : (
                      <Chip
                        label={t('principals.inactiveUser')}
                        size="small"
                        variant="outlined"
                      />
                    )}
                  </Detail>
                  <Detail label={t('users.memberSince')}>
                    <Timestamp style="date" value={user.member_since} />
                  </Detail>
                </DetailList>
              </Stack>
            </Paper>
            <Box component="section" aria-labelledby={teamsHeadingId}>
              <Typography
                component="h2"
                id={teamsHeadingId}
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
