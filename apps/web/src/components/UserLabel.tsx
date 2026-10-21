import { Box } from '@mui/material';
import { UserAvatar } from './UserAvatar';
import { UserProfileLink } from './UserProfileLink';
import { userAvatarSizes } from './userAvatars';

/**
 * A person's name led by their avatar, for tables and running text. With a
 * `userId` the name links to their profile.
 */
export const UserLabel = ({
  name,
  avatarFileId,
  userId,
}: {
  name: string;
  avatarFileId?: string | null;
  userId?: string | null;
}) => (
  <Box
    component="span"
    sx={{
      alignItems: 'center',
      display: 'inline-flex',
      gap: 1,
      maxWidth: '100%',
      verticalAlign: 'middle',
    }}
  >
    <UserAvatar
      avatarFileId={avatarFileId}
      name={name}
      size={userAvatarSizes.inline}
    />
    <Box component="span" sx={{ minWidth: 0, overflowWrap: 'break-word' }}>
      {userId ? (
        <UserProfileLink userId={userId}>{name}</UserProfileLink>
      ) : (
        name
      )}
    </Box>
  </Box>
);
