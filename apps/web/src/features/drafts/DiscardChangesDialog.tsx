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
  disableRestoreFocus,
  discardLabel,
  onDiscard,
  onExited,
  onKeep,
  open,
  title,
}: {
  description: string;
  /** For a discard that removes the button that opened the dialog. */
  disableRestoreFocus?: boolean;
  discardLabel: string;
  onDiscard: () => void;
  /** Runs once the dialog has closed, when the page can take focus again. */
  onExited?: () => void;
  onKeep: () => void;
  open: boolean;
  title: string;
}) => {
  const { t } = useTranslation();
  const descriptionId = useId();
  return (
    <Dialog
      aria-describedby={descriptionId}
      disableRestoreFocus={disableRestoreFocus}
      onClose={onKeep}
      open={open}
      slotProps={{ transition: { onExited } }}
    >
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
