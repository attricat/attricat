import {
  Box,
  CircularProgress,
  Typography,
  type CircularProgressProps,
  type SxProps,
  type Theme,
} from '@mui/material';
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

/** A centred spinner holding the place of content that is about to appear. */
export const LoadingSpinner = ({
  label,
  size,
  sx,
}: {
  label: string;
  size?: CircularProgressProps['size'];
  sx?: SxProps<Theme>;
}) => (
  <Box
    aria-live="polite"
    role="status"
    sx={[
      { display: 'flex', justifyContent: 'center' },
      ...(Array.isArray(sx) ? sx : [sx]),
    ]}
  >
    <CircularProgress aria-label={label} enableTrackSlot size={size} />
  </Box>
);
