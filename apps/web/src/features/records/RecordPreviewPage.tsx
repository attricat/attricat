import { useNavigate } from '@tanstack/react-router';
import { useTranslation } from 'react-i18next';
import { PageContainer } from '../../components/PageContainer';
import { PageHeader } from '../../components/PageHeader';
import { RecordPageEyebrow } from './components/RecordPageEyebrow';
import { RecordPreview } from './components/RecordPreview';

export const RecordPreviewPage = ({
  recordId,
  relationshipPickerToken,
}: {
  recordId: string;
  relationshipPickerToken?: string;
}) => {
  const { t } = useTranslation();
  const navigate = useNavigate();
  return (
    <RecordPreview
      recordId={recordId}
      // Context, dialogs and unsaved edits belong to one record.
      key={recordId}
      onDeleted={() => void navigate({ to: '/' })}
      onDuplicated={(copy) =>
        void navigate({
          params: { recordId: copy.id },
          to: '/records/$recordId',
        })
      }
      relationshipPickerToken={relationshipPickerToken}
      renderFrame={({ blueprint, children, headerActions }) => (
        <PageContainer>
          <PageHeader
            actions={headerActions}
            eyebrow={
              <RecordPageEyebrow
                blueprint={blueprint}
                label={t('records.recordPreview')}
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
