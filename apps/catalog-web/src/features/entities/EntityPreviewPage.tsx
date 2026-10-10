import { useNavigate } from '@tanstack/react-router';
import { useTranslation } from 'react-i18next';
import { PageContainer } from '../../components/PageContainer';
import { PageHeader } from '../../components/PageHeader';
import { EntityPageEyebrow } from './components/EntityPageEyebrow';
import { EntityPreview } from './components/EntityPreview';

export const EntityPreviewPage = ({
  entityId,
  relationshipPickerToken,
}: {
  entityId: string;
  relationshipPickerToken?: string;
}) => {
  const { t } = useTranslation();
  const navigate = useNavigate();
  return (
    <EntityPreview
      entityId={entityId}
      // Context, dialogs and unsaved edits belong to one record.
      key={entityId}
      onDeleted={() => void navigate({ to: '/' })}
      onDuplicated={(copy) =>
        void navigate({
          params: { entityId: copy.id },
          to: '/entities/$entityId',
        })
      }
      relationshipPickerToken={relationshipPickerToken}
      renderFrame={({ blueprint, children, headerActions }) => (
        <PageContainer>
          <PageHeader
            actions={headerActions}
            eyebrow={
              <EntityPageEyebrow
                blueprint={blueprint}
                label={t('entities.entityPreview')}
              />
            }
          />
          {children}
        </PageContainer>
      )}
      showComments
    />
  );
};
