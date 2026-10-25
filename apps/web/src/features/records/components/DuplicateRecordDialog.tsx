import { useMutation, useQueryClient } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import { duplicateRecord, type AttricatRecord } from '../api';
import { invalidateRecordSearches } from '../invalidateRecord';
import { ConfirmDialog } from '../../../components/ConfirmDialog';

/** Confirms copying a record, then reports the copy. */
export const DuplicateRecordDialog = ({
  recordId,
  onClose,
  onDuplicated,
}: {
  recordId: string;
  onClose: () => void;
  onDuplicated: (copy: AttricatRecord) => void;
}) => {
  const { t } = useTranslation();
  const client = useQueryClient();
  const duplication = useMutation({
    mutationFn: () => duplicateRecord(recordId),
    onSuccess: (copy) => {
      void invalidateRecordSearches(client);
      onDuplicated(copy);
    },
  });

  return (
    <ConfirmDialog
      confirmLabel={t('records.duplicateRecord')}
      error={duplication.error}
      isPending={duplication.isPending}
      onClose={onClose}
      onConfirm={() => duplication.mutate()}
      title={t('records.duplicateRecord')}
    >
      {t('records.duplicateRecordConfirmation')}
    </ConfirmDialog>
  );
};
