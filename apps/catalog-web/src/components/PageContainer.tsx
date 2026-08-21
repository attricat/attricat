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
    maxWidth={maxWidth}
    sx={{ ml: 0, mr: 'auto', py: { xs: 2, md: 3 } }}
  >
    {children}
  </Container>
);
