import { Box, Card, Typography } from '@mui/material';
import type { ReactNode } from 'react';

/**
 * One image file on a card of its own: a square preview that fills the card,
 * its name on one line, and its actions at the foot. Cards in a grid row
 * stretch to the same height, so galleries and single images line up.
 */
export const FileCard = ({
  actions,
  children,
  filename,
  preview,
}: {
  actions?: ReactNode;
  /** Anything shown below the actions, such as a file's panel. */
  children?: ReactNode;
  filename: string;
  preview: ReactNode;
}) => (
  <Card
    sx={{
      display: 'flex',
      flexDirection: 'column',
      gap: 1,
      height: '100%',
      minWidth: 0,
      p: 1,
    }}
  >
    {preview}
    <Typography noWrap title={filename} variant="caption">
      {filename}
    </Typography>
    {actions && <Box sx={{ mt: 'auto' }}>{actions}</Box>}
    {children}
  </Card>
);
