import { Container } from '@mui/material';
import type { ReactNode } from 'react';

export const PageContainer = ({
  children,
  maxWidth = 'xl',
}: {
  children: ReactNode;
  maxWidth?: 'sm' | 'md' | 'lg' | 'xl';
}) => (
  <Container
    maxWidth={maxWidth}
    sx={{ ml: 0, mr: 'auto', py: { xs: 3, md: 5 } }}
  >
    {children}
  </Container>
);
