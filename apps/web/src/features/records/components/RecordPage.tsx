import { Link } from '@tanstack/react-router';
import type { ReactNode } from 'react';
import { PageContainer } from '../../../components/PageContainer';
import { PageHeader } from '../../../components/PageHeader';
import { RecordIcon } from '../../../components/systemIcons';
import { lexiconText } from '../../lexicon/lexicon';

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
            <Link
              params={{ blueprintId: blueprint.id }}
              style={{ textDecoration: 'none' }}
              to="/manage/blueprints/$blueprintId"
            >
              {lexiconText(blueprint.name)}
            </Link>
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
