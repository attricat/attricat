import { Box, Typography, type SxProps, type Theme } from '@mui/material';
import type { ReactNode } from 'react';
import { monoFontFamily } from '../app/theme';

/** Labelled values, side by side where there is room. Holds `Detail`s. */
export const DetailList = ({
  children,
  sx,
}: {
  children: ReactNode;
  sx?: SxProps<Theme>;
}) => (
  <Box
    component="dl"
    sx={[
      {
        alignItems: 'baseline',
        columnGap: 4,
        display: 'grid',
        gridTemplateColumns: { sm: 'max-content 1fr' },
        m: 0,
        minWidth: 0,
        rowGap: 1,
      },
      ...(Array.isArray(sx) ? sx : [sx]),
    ]}
  >
    {children}
  </Box>
);

export const Detail = ({
  children,
  label,
  mono = false,
}: {
  children: ReactNode;
  label: ReactNode;
  /** For identifiers and versions. */
  mono?: boolean;
}) => (
  <>
    <Typography color="text.secondary" component="dt" variant="subtitle2">
      {label}
    </Typography>
    <Typography
      component="dd"
      sx={[
        { m: 0, minWidth: 0, overflowWrap: 'anywhere' },
        mono && { fontFamily: monoFontFamily },
      ]}
    >
      {children}
    </Typography>
  </>
);
