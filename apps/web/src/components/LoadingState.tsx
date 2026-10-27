import { Typography, type SxProps, type Theme } from '@mui/material';
import type { ReactNode } from 'react';

/** Says what is loading; screen readers announce it politely. */
export const LoadingState = ({
  children,
  sx,
}: {
  children: ReactNode;
  sx?: SxProps<Theme>;
}) => (
  <Typography role="status" sx={sx}>
    {children}
  </Typography>
);
