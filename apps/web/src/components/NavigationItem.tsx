import { Link } from '@tanstack/react-router';
import {
  Box,
  ListItemButton,
  ListItemIcon,
  ListItemText,
  Tooltip,
  Typography,
  type SxProps,
  type Theme,
} from '@mui/material';
import type { ReactNode } from 'react';
import {
  compactNavigationItemSx,
  compactNavigationLabelSx,
} from './sideNavigationLayout';

type NavigationItemProps = {
  /** The accessible name when it should say more than `label`. */
  accessibleLabel?: string;
  ariaControls?: string;
  ariaExpanded?: boolean;
  compact: boolean;
  /** Disables the item and says why in its tooltip. */
  disabledReason?: string;
  // External destination opened in a new tab; takes precedence over `to`.
  href?: string;
  icon: ReactNode;
  label: string;
  onClick?: () => void;
  selected?: boolean;
  sx?: SxProps<Theme>;
  to?: string;
  trailing?: ReactNode;
};

export const NavigationItem = ({
  accessibleLabel,
  ariaControls,
  ariaExpanded,
  compact,
  disabledReason,
  href,
  icon,
  label,
  onClick,
  selected,
  sx,
  to,
  trailing,
}: NavigationItemProps) => {
  const buttonProps = {
    'aria-controls': ariaControls,
    'aria-expanded': ariaExpanded,
    'aria-label': accessibleLabel ?? label,
    onClick,
    selected,
    sx: compact ? compactNavigationItemSx : sx,
  };
  const content = (
    <>
      <ListItemIcon sx={compact ? { minWidth: 0 } : undefined}>
        {icon}
      </ListItemIcon>
      {compact ? (
        <Typography sx={compactNavigationLabelSx} variant="caption">
          {label}
        </Typography>
      ) : (
        <ListItemText primary={label} />
      )}
      {trailing}
    </>
  );

  if (disabledReason) {
    return (
      <Tooltip placement="right" title={disabledReason}>
        <Box component="span" sx={{ display: 'block', width: '100%' }}>
          <ListItemButton {...buttonProps} disabled>
            {content}
          </ListItemButton>
        </Box>
      </Tooltip>
    );
  }

  return (
    <Tooltip placement="right" title={compact ? label : ''}>
      {href ? (
        <ListItemButton
          {...buttonProps}
          component="a"
          href={href}
          rel="noopener noreferrer"
          target="_blank"
        >
          {content}
        </ListItemButton>
      ) : to ? (
        <ListItemButton {...buttonProps} component={Link} to={to}>
          {content}
        </ListItemButton>
      ) : (
        <ListItemButton {...buttonProps}>{content}</ListItemButton>
      )}
    </Tooltip>
  );
};
