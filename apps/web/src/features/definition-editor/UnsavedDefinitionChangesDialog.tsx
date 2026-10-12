import {
  Button,
  Dialog,
  DialogActions,
  DialogContent,
  DialogContentText,
  DialogTitle,
} from '@mui/material';
import { useId } from 'react';
import { useTranslation } from 'react-i18next';

export type UnsavedChangesAction = 'replace' | 'discard';

/** Confirms replacing or discarding unsaved definition edits. */
export const UnsavedDefinitionChangesDialog = ({
  action,
  onCancel,
  onConfirm,
}: {
  action: UnsavedChangesAction | undefined;
  onCancel: () => void;
  onConfirm: () => void;
}) => {
  const { t } = useTranslation();
  const descriptionId = useId();
  const replacing = action === 'replace';
  return (
    <Dialog
      aria-describedby={descriptionId}
      onClose={onCancel}
      open={action !== undefined}
    >
      <DialogTitle>{t('definitionEditor.unsaved.title')}</DialogTitle>
      <DialogContent>
        <DialogContentText id={descriptionId}>
          {replacing
            ? t('definitionEditor.unsaved.replaceQuestion')
            : t('definitionEditor.unsaved.discardQuestion')}
        </DialogContentText>
      </DialogContent>
      <DialogActions>
        <Button onClick={onCancel}>
          {t('definitionEditor.unsaved.keepEditing')}
        </Button>
        <Button color="error" onClick={onConfirm} variant="contained">
          {replacing
            ? t('definitionEditor.unsaved.replace')
            : t('definitionEditor.unsaved.discard')}
        </Button>
      </DialogActions>
    </Dialog>
  );
};
