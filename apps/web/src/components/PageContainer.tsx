import { Container } from '@mui/material';
import type { ReactNode } from 'react';

export const PageContainer = ({
  children,
  maxWidth = false,
}: {
  children: ReactNode;
  maxWidth?: false | 'sm' | 'md' | 'lg' | 'xl';
}) => (
  <Container
    disableGutters
    maxWidth={maxWidth}
    sx={{ ml: 0, mr: 'auto', px: { xs: 4, md: 8 }, py: { xs: 6, md: 8 } }}
  >
    {children}
  </Container>
);
