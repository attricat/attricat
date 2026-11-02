import {
  ListItemIcon,
  ListItemText,
  MenuItem,
  type MenuItemProps,
} from '@mui/material';
import type { LucideIcon } from 'lucide-react';
import type { ReactNode } from 'react';
import { compactIconSize } from './iconSizes';

const toneColors = {
  destructive: 'error.main',
  reversal: 'warning.main',
} as const;

type ActionMenuItemTone = keyof typeof toneColors;

type ContentProps = {
  children: ReactNode;
  icon: LucideIcon;
  /** Explains the action, such as why it is disabled. */
  secondary?: ReactNode;
  tone?: ActionMenuItemTone;
};

/** An action menu item's icon and label, for a `RouterMenuItem`. */
export const ActionMenuItemContent = ({
  children,
  icon: Icon,
  secondary,
  tone,
}: ContentProps) => (
  <>
    <ListItemIcon sx={tone && { color: 'inherit' }}>
      <Icon size={compactIconSize} />
    </ListItemIcon>
    <ListItemText secondary={secondary}>{children}</ListItemText>
  </>
);

/** One item of an action menu; `tone` colours deletions and reversals. */
export const ActionMenuItem = ({
  children,
  icon,
  secondary,
  tone,
  ...props
}: ContentProps & Omit<MenuItemProps, 'children' | 'sx'>) => (
  <MenuItem {...props} sx={tone && { color: toneColors[tone] }}>
    <ActionMenuItemContent icon={icon} secondary={secondary} tone={tone}>
      {children}
    </ActionMenuItemContent>
  </MenuItem>
);
