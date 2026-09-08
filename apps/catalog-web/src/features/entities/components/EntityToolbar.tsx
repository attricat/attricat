import { Paper } from '@mui/material';
import type { ReactNode } from 'react';

type Props = {
  children: ReactNode;
  label: string;
};

/** Shared toolbar shell; each entity page composes its own relevant actions. */
export const EntityToolbar = ({ children, label }: Props) => (
  <Paper
    aria-label={label}
    component="nav"
    sx={{
      alignItems: 'center',
      display: 'flex',
      flexWrap: 'wrap',
      gap: 1,
      mt: 3,
      p: 1.5,
    }}
  >
    {children}
  </Paper>
);
