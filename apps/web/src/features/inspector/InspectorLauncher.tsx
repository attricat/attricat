import { useEffect, useState } from 'react';
import { Box, Button, ClickAwayListener } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { InspectorIcon } from '../../components/systemIcons';
import {
  apiHealthColor,
  apiHealthSummaryKeys,
  type ApiHealthState,
} from './apiHealth';
import {
  LAUNCHER_TOUCH_REVEAL_MS,
  launcherHandleHeight,
  launcherHandleOffset,
  launcherHandleWidth,
  launcherLoadingAnimationDuration,
  launcherLoadingAnimationName,
  launcherRevealZoneHeight,
  launcherRevealZoneTouchHeight,
  launcherRevealZoneWidth,
  launcherStatusDotOffset,
  launcherStatusDotSize,
} from './constants';
import { smallIconSize } from '../../components/iconSizes';
import radius from '../../../design/tokens/radius.json';

const revealedLauncher = { transform: 'translate(-50%, 0)' };

/**
 * Inspector button tucked below the viewport behind a small handle at the
 * bottom edge. Hovering the handle reveals it; touch devices reveal it with a tap and
 * hide it again after a short delay or a tap elsewhere. Keyboard focus reveals
 * it as well, so the button stays reachable with Tab.
 */
export const InspectorLauncher = ({
  apiHealth,
  onOpen,
}: {
  apiHealth: ApiHealthState;
  onOpen: () => void;
}) => {
  const { t } = useTranslation();
  const [tapRevealed, setTapRevealed] = useState(false);

  useEffect(() => {
    if (!tapRevealed) return;
    const timeout = setTimeout(
      () => setTapRevealed(false),
      LAUNCHER_TOUCH_REVEAL_MS,
    );
    return () => clearTimeout(timeout);
  }, [tapRevealed]);

  return (
    <ClickAwayListener onClickAway={() => setTapRevealed(false)}>
      <Box
        data-revealed={tapRevealed || undefined}
        onClick={() => setTapRevealed(true)}
        sx={(theme) => ({
          bottom: 0,
          height: launcherRevealZoneHeight,
          left: '50%',
          position: 'fixed',
          transform: 'translateX(-50%)',
          width: launcherRevealZoneWidth,
          zIndex: theme.zIndex.modal + 1,
          '@media (pointer: coarse)': {
            height: launcherRevealZoneTouchHeight,
          },
          // Touch browsers emulate a sticky :hover, so only real hover reveals.
          '@media (hover: hover)': {
            '&:hover > button': revealedLauncher,
            '&:hover > [aria-hidden]': {
              backgroundColor:
                apiHealth === 'ready' ? 'success.dark' : 'warning.dark',
            },
          },
          '&:focus-within > button, &[data-revealed] > button':
            revealedLauncher,
        })}
      >
        <Box
          aria-hidden
          sx={(theme) => ({
            backgroundColor: apiHealthColor(apiHealth),
            borderRadius: radius.pill,
            bottom: launcherHandleOffset,
            height: launcherHandleHeight,
            left: '50%',
            position: 'absolute',
            transform: 'translateX(-50%)',
            transition: theme.transitions.create('background-color', {
              duration: theme.transitions.duration.shorter,
            }),
            width: launcherHandleWidth,
            ...(apiHealth !== 'ready' && {
              overflow: 'hidden',
              [`@keyframes ${launcherLoadingAnimationName}`]: {
                from: { transform: 'translateX(-100%)' },
                to: { transform: `translateX(${launcherHandleWidth}px)` },
              },
              '&::after': {
                animation: `${launcherLoadingAnimationName} ${launcherLoadingAnimationDuration} infinite ease-in-out`,
                background: `linear-gradient(90deg, transparent, ${theme.palette.warning.light}, transparent)`,
                content: '""',
                inset: 0,
                position: 'absolute',
                width: '50%',
              },
              '@media (prefers-reduced-motion: reduce)': {
                '&::after': { animation: 'none', display: 'none' },
              },
            }),
          })}
        />
        <Button
          aria-label={t('inspector.open')}
          color="inherit"
          onClick={onOpen}
          size="small"
          startIcon={<InspectorIcon size={smallIconSize} />}
          variant="outlined"
          sx={(theme) => ({
            backgroundColor: 'background.paper',
            bottom: '100%',
            boxShadow: 4,
            color: 'text.secondary',
            left: '50%',
            position: 'absolute',
            transform: `translate(-50%, calc(100% + ${launcherRevealZoneTouchHeight}px))`,
            transition: theme.transitions.create('transform', {
              duration: theme.transitions.duration.shorter,
            }),
            whiteSpace: 'nowrap',
            '&:hover': {
              backgroundColor: 'background.paper',
              color: 'primary.main',
            },
            '@media (prefers-reduced-motion: reduce)': {
              transition: 'none',
            },
          })}
        >
          {t('inspector.open')}
          <Box
            aria-label={t(apiHealthSummaryKeys[apiHealth])}
            role="status"
            sx={{
              backgroundColor: apiHealthColor(apiHealth),
              border: 2,
              borderColor: 'background.paper',
              borderRadius: '50%',
              height: launcherStatusDotSize,
              position: 'absolute',
              right: -launcherStatusDotOffset,
              top: -launcherStatusDotOffset,
              width: launcherStatusDotSize,
            }}
          />
        </Button>
      </Box>
    </ClickAwayListener>
  );
};
