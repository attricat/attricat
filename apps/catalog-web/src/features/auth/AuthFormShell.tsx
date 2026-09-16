import { Paper, Stack } from '@mui/material';
import type { ReactNode } from 'react';

export const authFormWidth = 360;

export const AuthFormShell = ({
  children,
  header,
  onSubmit,
}: {
  children: ReactNode;
  header?: ReactNode;
  onSubmit: () => void;
}) => (
  <Stack
    sx={{ alignItems: 'center', justifyContent: 'center', minHeight: '100dvh' }}
  >
    {header}
    <Paper
      component="form"
      onSubmit={(event) => {
        event.preventDefault();
        onSubmit();
      }}
      sx={{ p: 4, width: authFormWidth }}
    >
      <Stack spacing={2}>{children}</Stack>
    </Paper>
  </Stack>
);
