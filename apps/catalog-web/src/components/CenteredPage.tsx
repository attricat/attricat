import { Box } from '@mui/material';
import type { ReactNode } from 'react';

const narrowPageMaxWidth = 500;
const settingsPageMaxWidth = 1000;

export const NarrowPage = ({ children }: { children: ReactNode }) => (
  <Box sx={{ maxWidth: narrowPageMaxWidth, mx: 'auto', p: 3 }}>{children}</Box>
);

export const SettingsPage = ({ children }: { children: ReactNode }) => (
  <Box sx={{ maxWidth: settingsPageMaxWidth, mx: 'auto', p: 3 }}>
    {children}
  </Box>
);
