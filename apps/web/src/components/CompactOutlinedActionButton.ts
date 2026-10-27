import type { SxProps, Theme } from '@mui/material';
import { pillRadius } from '../app/theme';

const compactActionButtonHeight = 24;

export const compactOutlinedActionButtonSx = {
  borderRadius: `${pillRadius}px`,
  flexShrink: 0,
  height: compactActionButtonHeight,
  minHeight: compactActionButtonHeight,
  px: 1,
  '& .MuiButton-startIcon': { mr: 0.5 },
} satisfies SxProps<Theme>;
