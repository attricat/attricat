import { Box, Stack, Typography } from '@mui/material';
import type { ReactNode } from 'react';

export const PageHeader = ({
  actions,
  description,
  eyebrow,
  title,
  titleVariant = 'h2',
}: {
  actions?: ReactNode;
  description?: ReactNode;
  eyebrow?: string;
  title?: ReactNode;
  titleVariant?: 'h2' | 'h3';
}) => (
  <Stack
    direction={{ xs: 'column', sm: actions ? 'row' : 'column' }}
    spacing={2}
    sx={{ justifyContent: 'space-between' }}
  >
    <Box>
      {eyebrow && (
        <Typography
          color="primary"
          sx={{
            fontWeight: 700,
            letterSpacing: '.12em',
            textTransform: 'uppercase',
          }}
          variant="overline"
        >
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
