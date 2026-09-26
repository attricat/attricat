import { Box, Stack, Typography } from '@mui/material';
import type { ReactNode } from 'react';

export const PageHeader = ({
  actions,
  description,
  eyebrow,
  title,
  titleVariant = 'h3',
}: {
  actions?: ReactNode;
  description?: ReactNode;
  eyebrow?: ReactNode;
  title?: ReactNode;
  titleVariant?: 'h2' | 'h3';
}) => (
  <Stack
    direction={{ xs: 'column', sm: actions ? 'row' : 'column' }}
    spacing={1.5}
    sx={{ justifyContent: 'space-between' }}
  >
    <Box>
      {eyebrow && (
        <Typography color="primary" variant="overline">
          {eyebrow}
        </Typography>
      )}
      {title && (
        <Typography component="h1" variant={titleVariant}>
          {title}
        </Typography>
      )}
      {description && (
        <Typography color="text.secondary">{description}</Typography>
      )}
    </Box>
    {actions && <Box sx={{ alignSelf: { sm: 'center' } }}>{actions}</Box>}
  </Stack>
);
