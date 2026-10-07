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
import { duplicateEntity, type Entity } from '../api';
import { invalidateEntitySearches } from '../invalidateEntity';

/** Confirms copying an entity, then reports the copy. */
export const DuplicateEntityDialog = ({
  entityId,
  onClose,
  onDuplicated,
}: {
  entityId: string;
  onClose: () => void;
  onDuplicated: (copy: Entity) => void;
}) => {
  const { t } = useTranslation();
  const client = useQueryClient();
  const duplication = useMutation({
    mutationFn: () => duplicateEntity(entityId),
    onSuccess: (copy) => {
      void invalidateEntitySearches(client);
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
      <DialogTitle>{t('entities.duplicateEntity')}</DialogTitle>
      <DialogContent>
        <Typography>{t('entities.duplicateEntityConfirmation')}</Typography>
        {duplication.isError && (
          <Alert severity="error" sx={{ mt: 2 }}>
            {duplication.error.message}
          </Alert>
        )}
      </DialogContent>
      <DialogActions>
        <Button disabled={duplication.isPending} onClick={onClose}>
          {t('entities.cancel')}
        </Button>
        <Button
          disabled={duplication.isPending}
          onClick={() => duplication.mutate()}
          variant="contained"
        >
          {t('entities.duplicateEntity')}
        </Button>
      </DialogActions>
    </Dialog>
  );
};
