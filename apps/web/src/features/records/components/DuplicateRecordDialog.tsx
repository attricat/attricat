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
import { duplicateRecord, type AttricatRecord } from '../api';
import { invalidateRecordSearches } from '../invalidateRecord';

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
    <Dialog
      open
      onClose={duplication.isPending ? undefined : onClose}
      fullWidth
      maxWidth="xs"
    >
      <DialogTitle>{t('records.duplicateRecord')}</DialogTitle>
      <DialogContent>
        <Typography>{t('records.duplicateRecordConfirmation')}</Typography>
        {duplication.isError && (
          <Alert severity="error" sx={{ mt: 2 }}>
            {duplication.error.message}
          </Alert>
        )}
      </DialogContent>
      <DialogActions>
        <Button disabled={duplication.isPending} onClick={onClose}>
          {t('records.cancel')}
        </Button>
        <Button
          disabled={duplication.isPending}
          onClick={() => duplication.mutate()}
          variant="contained"
        >
          {t('records.duplicateRecord')}
        </Button>
      </DialogActions>
    </Dialog>
  );
};
