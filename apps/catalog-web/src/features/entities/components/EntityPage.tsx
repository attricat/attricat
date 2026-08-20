import type { ReactNode } from 'react';
import { PageContainer } from '../../../components/PageContainer';
import { PageHeader } from '../../../components/PageHeader';

export const EntityPage = ({
  children,
  title,
}: {
  children: ReactNode;
  title: string;
}) => {
  return (
    <PageContainer maxWidth="md">
      <PageHeader title={title} titleVariant="h3" />
      {children}
    </PageContainer>
  );
};
