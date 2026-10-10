import type { SxProps, Theme } from '@mui/material';

const pillBorderRadius = 999;
const compactActionButtonHeight = 24;

export const compactOutlinedActionButtonSx = {
  borderRadius: pillBorderRadius,
  flexShrink: 0,
  height: compactActionButtonHeight,
  minHeight: compactActionButtonHeight,
  px: 1,
  '& .MuiButton-startIcon': { mr: 0.5 },
} satisfies SxProps<Theme>;
