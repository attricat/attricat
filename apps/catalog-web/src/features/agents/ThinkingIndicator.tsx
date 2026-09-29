import { Avatar, Box, Stack, Typography } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { AgentIcon } from '../../components/systemIcons';
import {
  avatarSize,
  thinkingAnimationDuration,
  thinkingAnimationName,
  thinkingDotDelaysSeconds,
} from './constants';
import { smallIconSize } from '../../components/iconSizes';

const dotStyles = Object.fromEntries(
  thinkingDotDelaysSeconds.map((delay, index) => [
    `& span:nth-of-type(${index + 1})`,
    { animationDelay: `${delay}s` },
  ]),
);

export const ThinkingIndicator = () => {
  const { t } = useTranslation();
  return (
    <Stack
      aria-live="polite"
      direction="row"
      role="status"
      spacing={1.5}
      sx={{ alignItems: 'center' }}
    >
      <Avatar
        sx={{ bgcolor: 'primary.main', height: avatarSize, width: avatarSize }}
      >
        <AgentIcon size={smallIconSize} />
      </Avatar>
      <Typography color="text.secondary" variant="body2">
        {t('agents.thinking')}
        <Box
          component="span"
          sx={{
            [`@keyframes ${thinkingAnimationName}`]: {
              '0%, 80%, 100%': { opacity: 0.25 },
              '40%': { opacity: 1 },
            },
            '& span': {
              animation: `${thinkingAnimationName} ${thinkingAnimationDuration} infinite ease-in-out`,
              display: 'inline-block',
              ml: 0.25,
            },
            ...dotStyles,
          }}
        >
          {thinkingDotDelaysSeconds.map((delay) => (
            <span key={delay}>•</span>
          ))}
        </Box>
      </Typography>
    </Stack>
  );
};
