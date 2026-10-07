import { Button, IconButton, ListItemButton } from '@mui/material';
import { createLink } from '@tanstack/react-router';
import { AnchorChip } from './AnchorChip';

export const RouterButton = createLink(Button);
export const RouterChip = createLink(AnchorChip);
export const RouterIconButton = createLink(IconButton);
export const RouterListItemButton = createLink(ListItemButton);
