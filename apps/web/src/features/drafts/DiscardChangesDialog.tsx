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

/** Asks before dropping unsaved changes; closing it keeps editing. */
export const DiscardChangesDialog = ({
  description,
  discardLabel,
  onDiscard,
  onKeep,
  open,
  title,
}: {
  description: string;
  discardLabel: string;
  onDiscard: () => void;
  onKeep: () => void;
  open: boolean;
  title: string;
}) => {
  const { t } = useTranslation();
  const descriptionId = useId();
  return (
    <Dialog aria-describedby={descriptionId} onClose={onKeep} open={open}>
      <DialogTitle>{title}</DialogTitle>
      <DialogContent>
        <DialogContentText id={descriptionId}>{description}</DialogContentText>
      </DialogContent>
      <DialogActions>
        <Button onClick={onKeep}>{t('drafts.keepEditing')}</Button>
        <Button color="error" onClick={onDiscard} variant="contained">
          {discardLabel}
        </Button>
      </DialogActions>
    </Dialog>
  );
};
