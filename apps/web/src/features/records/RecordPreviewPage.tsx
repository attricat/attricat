import { useQuery } from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';
import { useTranslation } from 'react-i18next';
import { useResourcePageTitle } from '../../app/useResourcePageTitle';
import { PageContainer } from '../../components/PageContainer';
import { PageHeader } from '../../components/PageHeader';
import { RecordPageEyebrow } from './components/RecordPageEyebrow';
import { RecordPreview } from './components/RecordPreview';
import { recordLabelOptions } from './queryOptions';

/**
 * Names the browser tab after the record's `page_title` view, then its
 * `dropdown_option` label. Records with neither keep the route's short-ID
 * title.
 */
const RecordDocumentTitle = ({
  recordId,
  title,
}: {
  recordId: string;
  title: string | null | undefined;
}) => {
  const label = useQuery({
    ...recordLabelOptions(recordId),
    enabled: title === null,
  });
  const fallback =
    label.data?.items.find((item) => item.id === recordId)?.display.default ||
    undefined;
  useResourcePageTitle(title === null ? fallback : title);
  return null;
};

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
      renderFrame={({ blueprint, children, headerActions, title }) => (
        <PageContainer>
          <RecordDocumentTitle recordId={recordId} title={title} />
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
