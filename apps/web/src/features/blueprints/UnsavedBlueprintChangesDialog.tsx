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

export const UnsavedBlueprintChangesDialog = ({
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
      <DialogTitle>{t('blueprints.unsavedChangesTitle')}</DialogTitle>
      <DialogContent>
        <DialogContentText id={descriptionId}>
          {replacing
            ? t('blueprints.replaceUnsavedChanges')
            : t('blueprints.discardUnsavedChanges')}
        </DialogContentText>
      </DialogContent>
      <DialogActions>
        <Button onClick={onCancel}>{t('blueprints.keepEditing')}</Button>
        <Button color="error" onClick={onConfirm} variant="contained">
          {replacing ? t('blueprints.replace') : t('blueprints.discard')}
        </Button>
      </DialogActions>
    </Dialog>
  );
};
