import { useMutation, useQueryClient } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import { deleteRecord } from '../api';
import { removeRecord } from '../invalidateRecord';
import { ConfirmDialog } from '../../../components/ConfirmDialog';

export const DeleteRecordDialog = ({
  recordId,
  onClose,
  onDeleted,
}: {
  recordId: string;
  onClose: () => void;
  /** Leaves the deleted record; may return a promise that settles once it has. */
  onDeleted: () => unknown;
}) => {
  const { t } = useTranslation();
  const client = useQueryClient();
  const deletion = useMutation({
    mutationFn: () => deleteRecord(recordId),
    onSuccess: async () => {
      // Leave first: views of the record still mounted would refetch it,
      // and every read of a deleted record fails.
      await onDeleted();
      void removeRecord(client, recordId);
    },
  });

  return (
    <ConfirmDialog
      confirmLabel={t('records.deleteRecord')}
      destructive
      error={deletion.error}
      isPending={deletion.isPending}
      onClose={onClose}
      onConfirm={() => deletion.mutate()}
      title={t('records.deleteRecord')}
    >
      {t('records.deleteRecordConfirmation')}
    </ConfirmDialog>
  );
};
