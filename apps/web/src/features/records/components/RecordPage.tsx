import type { ReactNode } from 'react';
import { PageContainer } from '../../../components/PageContainer';
import { PageHeader } from '../../../components/PageHeader';
import { RecordIcon } from '../../../components/systemIcons';
import { BlueprintLink } from '../../blueprints/BlueprintLink';

export const RecordPage = ({
  blueprint,
  children,
  fullWidth = false,
  title,
}: {
  /** Named in the eyebrow, linking to its blueprint page. */
  blueprint?: { id: string; name: string };
  children: ReactNode;
  fullWidth?: boolean;
  title: string;
}) => {
  return (
    <PageContainer maxWidth={fullWidth ? false : 'md'}>
      <PageHeader
        eyebrow={
          blueprint && (
            <BlueprintLink
              blueprint={blueprint}
              style={{ textDecoration: 'none' }}
            />
          )
        }
        icon={RecordIcon}
        title={title}
        titleVariant="h3"
      />
      {children}
    </PageContainer>
  );
};
