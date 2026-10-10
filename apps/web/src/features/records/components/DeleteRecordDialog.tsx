import { useMutation, useQueryClient } from '@tanstack/react-query';
import {
  Alert,
  Button,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  Typography,
} from '@mui/material';
import { useTranslation } from 'react-i18next';
import { deleteRecord } from '../api';
import { removeRecord } from '../invalidateRecord';

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
    <Dialog
      open
      onClose={deletion.isPending ? undefined : onClose}
      fullWidth
      maxWidth="xs"
    >
      <DialogTitle>{t('records.deleteRecord')}</DialogTitle>
      <DialogContent>
        <Typography>{t('records.deleteRecordConfirmation')}</Typography>
        {deletion.isError && (
          <Alert severity="error" sx={{ mt: 2 }}>
            {deletion.error.message}
          </Alert>
        )}
      </DialogContent>
      <DialogActions>
        <Button disabled={deletion.isPending} onClick={onClose}>
          {t('records.cancel')}
        </Button>
        <Button
          color="error"
          disabled={deletion.isPending}
          onClick={() => deletion.mutate()}
          variant="contained"
        >
          {t('records.deleteRecord')}
        </Button>
      </DialogActions>
    </Dialog>
  );
};
