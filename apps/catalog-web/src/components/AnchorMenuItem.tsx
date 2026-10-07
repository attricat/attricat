import { MenuItem, type MenuItemProps } from '@mui/material';
import { forwardRef } from 'react';

/** A menu item rendered as a link element, for use with router links. */
export const AnchorMenuItem = forwardRef<HTMLAnchorElement, MenuItemProps<'a'>>(
  (props, ref) => <MenuItem component="a" ref={ref} {...props} />,
);
