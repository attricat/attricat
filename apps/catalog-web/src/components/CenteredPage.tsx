import { Box } from '@mui/material';
import type { ReactNode } from 'react';

export const NarrowPage = ({ children }: { children: ReactNode }) => (
  <Box sx={{ maxWidth: 500, mx: 'auto', p: 3 }}>{children}</Box>
);

export const SettingsPage = ({ children }: { children: ReactNode }) => (
  <Box sx={{ maxWidth: 1000, mx: 'auto', p: 3 }}>{children}</Box>
);
