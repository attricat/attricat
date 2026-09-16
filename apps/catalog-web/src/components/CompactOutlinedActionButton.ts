import type { SxProps, Theme } from '@mui/material';

export const compactOutlinedActionButtonSx = {
  borderRadius: 999,
  flexShrink: 0,
  height: 24,
  minHeight: 24,
  px: 1,
  '& .MuiButton-startIcon': { mr: 0.5 },
} satisfies SxProps<Theme>;
