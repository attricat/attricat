import { Box, Typography } from '@mui/material';
import type { LucideIcon } from 'lucide-react';
import { createElement, type ReactNode } from 'react';

export type PageTitleVariant = 'h2' | 'h3' | 'h4';

// The page's h1, prefixed with the icon of the system concept it presents.
// The icon scales with the heading's font size and is hidden from assistive
// technology, so the accessible heading name is the title alone.
export const PageTitle = ({
  children,
  icon,
  variant = 'h3',
}: {
  children: ReactNode;
  icon: LucideIcon;
  variant?: PageTitleVariant;
}) => (
  <Typography
    component="h1"
    sx={{ alignItems: 'center', display: 'flex', gap: 1.5 }}
    variant={variant}
  >
    <Box
      component="span"
      sx={{ color: 'primary.main', display: 'inline-flex', flexShrink: 0 }}
    >
      {createElement(icon)}
    </Box>
    <span>{children}</span>
  </Typography>
);
