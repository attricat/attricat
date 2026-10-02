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
import { deleteEntity } from '../api';
import { removeEntity } from '../invalidateEntity';

export const DeleteEntityDialog = ({
  entityId,
  onClose,
  onDeleted,
}: {
  entityId: string;
  onClose: () => void;
  onDeleted: () => void;
}) => {
  const { t } = useTranslation();
  const client = useQueryClient();
  const deletion = useMutation({
    mutationFn: () => deleteEntity(entityId),
    onSuccess: () => {
      void removeEntity(client, entityId);
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
      <DialogTitle>{t('entities.deleteEntity')}</DialogTitle>
      <DialogContent>
        <Typography>{t('entities.deleteEntityConfirmation')}</Typography>
        {deletion.isError && (
          <Alert severity="error" sx={{ mt: 2 }}>
            {deletion.error.message}
          </Alert>
        )}
      </DialogContent>
      <DialogActions>
        <Button disabled={deletion.isPending} onClick={onClose}>
          {t('entities.cancel')}
        </Button>
        <Button
          color="error"
          disabled={deletion.isPending}
          onClick={() => deletion.mutate()}
          variant="contained"
        >
          {t('entities.deleteEntity')}
        </Button>
      </DialogActions>
    </Dialog>
  );
};
