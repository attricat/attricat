import type { ReactNode } from 'react';
import { PageContainer } from '../../../components/PageContainer';
import { PageHeader } from '../../../components/PageHeader';

export const EntityPage = ({
  children,
  fullWidth = false,
  title,
}: {
  children: ReactNode;
  fullWidth?: boolean;
  title: string;
}) => {
  return (
    <PageContainer maxWidth={fullWidth ? false : 'md'}>
      <PageHeader title={title} titleVariant="h3" />
      {children}
    </PageContainer>
  );
};
