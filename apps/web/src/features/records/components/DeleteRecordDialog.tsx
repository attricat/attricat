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
  onDeleted: () => void;
}) => {
  const { t } = useTranslation();
  const client = useQueryClient();
  const deletion = useMutation({
    mutationFn: () => deleteRecord(recordId),
    onSuccess: () => {
      void removeRecord(client, recordId);
      onDeleted();
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
