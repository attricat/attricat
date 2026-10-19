import { Box, Stack } from '@mui/material';
import type { ReactNode } from 'react';

/** Keeps the bar above field labels and adornments that scroll beneath it. */
const actionBarLayer = 2;

/**
 * The actions of a long form, kept in view at the bottom of the screen. Place
 * it last inside the form's surface; `inset` is that surface's padding, which
 * the bar spans so its border meets the edges.
 */
export const FormActionBar = ({
  inset,
  status,
  children,
}: {
  inset: number;
  /** What still stands between the user and saving, shown before the actions. */
  status?: ReactNode;
  children: ReactNode;
}) => (
  <Box
    sx={{
      alignItems: 'center',
      bgcolor: 'background.paper',
      borderBottomLeftRadius: 'inherit',
      borderBottomRightRadius: 'inherit',
      // The shorthand would reset a separate border colour.
      borderTop: (theme) => `1px solid ${theme.palette.divider}`,
      bottom: 0,
      display: 'flex',
      flexWrap: 'wrap',
      gap: 2,
      mb: -inset,
      mx: -inset,
      position: 'sticky',
      px: inset,
      py: 2,
      zIndex: actionBarLayer,
    }}
  >
    <Box sx={{ flex: '1 1 auto', minWidth: 0 }}>{status}</Box>
    <Stack direction="row" spacing={1} sx={{ flexWrap: 'wrap' }} useFlexGap>
      {children}
    </Stack>
  </Box>
);
