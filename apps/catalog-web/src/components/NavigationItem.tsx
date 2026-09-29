import { Link } from '@tanstack/react-router';
import {
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
  ariaControls?: string;
  ariaExpanded?: boolean;
  compact: boolean;
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
  ariaControls,
  ariaExpanded,
  compact,
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
    'aria-label': label,
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
