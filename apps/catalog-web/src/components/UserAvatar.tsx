import { Avatar, type SxProps, type Theme } from '@mui/material';
import { fileDownloadUrl } from '../features/files/api';
import { AVATAR_VARIANT_KIND } from '../features/files/constants';
import { initialsFontScale, userInitials } from './userAvatars';

type UserAvatarProps = {
  /** Display name, falling back to email, used for the initials. */
  name?: string | null;
  /** A ready avatar file; initials show when absent or unloadable. */
  avatarFileId?: string | null;
  size: number;
  sx?: SxProps<Theme>;
};

/**
 * A person's avatar. It is decorative: callers render the person's name
 * alongside it, so the image has an empty text alternative.
 */
export const UserAvatar = ({
  name,
  avatarFileId,
  size,
  sx,
}: UserAvatarProps) => (
  <Avatar
    alt=""
    aria-hidden
    src={
      avatarFileId
        ? fileDownloadUrl(avatarFileId, AVATAR_VARIANT_KIND)
        : undefined
    }
    sx={[
      {
        bgcolor: 'action.selected',
        color: 'primary.main',
        fontSize: size * initialsFontScale,
        fontWeight: 600,
        height: size,
        width: size,
        // Transparent images always sit on white, as the file worker also
        // flattens new avatars, whatever the surrounding theme.
        '& img': { bgcolor: 'common.white' },
      },
      ...(Array.isArray(sx) ? sx : [sx]),
    ]}
  >
    {name ? userInitials(name) : undefined}
  </Avatar>
);
