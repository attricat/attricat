import { Box } from '@mui/material';
import type { ReactNode } from 'react';

/** Keeps text for assistive technology without drawing it. */
export const visuallyHiddenSx = {
  clip: 'rect(0 0 0 0)',
  height: '1px',
  overflow: 'hidden',
  position: 'absolute',
  whiteSpace: 'nowrap',
  width: '1px',
} as const;

export const VisuallyHidden = ({ children }: { children: ReactNode }) => (
  <Box component="span" sx={visuallyHiddenSx}>
    {children}
  </Box>
);
