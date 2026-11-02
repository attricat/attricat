import { Avatar } from '@mui/material';
import { smallIconSize } from '../../components/iconSizes';
import { AgentIcon } from '../../components/systemIcons';
import { userAvatarSizes } from '../../components/userAvatars';

/** Marks what the agent says or does in a conversation. */
export const AgentAvatar = () => (
  <Avatar
    sx={{
      bgcolor: 'primary.main',
      height: userAvatarSizes.message,
      width: userAvatarSizes.message,
    }}
  >
    <AgentIcon size={smallIconSize} />
  </Avatar>
);
