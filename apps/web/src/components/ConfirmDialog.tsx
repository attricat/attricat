import {
  Button,
  Dialog,
  DialogActions,
  DialogContent,
  DialogContentText,
  DialogTitle,
  type DialogProps,
} from '@mui/material';
import type { ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { ApiErrorAlert } from './ApiErrorAlert';

/**
 * Asks before an action and stays open, unable to close, while it runs; a
 * failure is shown above the buttons so the user can try again.
 */
export const ConfirmDialog = ({
  children,
  confirmLabel,
  confirmDisabled = false,
  destructive = false,
  error,
  isPending,
  maxWidth = 'xs',
  onClose,
  onConfirm,
  open = true,
  slotProps,
  title,
}: {
  /** The question; text is shown as dialog content text. */
  children: ReactNode;
  confirmLabel: ReactNode;
  /** Blocks confirming, such as when the field it changes is disabled; cancelling still works. */
  confirmDisabled?: boolean;
  destructive?: boolean;
  error: Error | null | undefined;
  isPending: boolean;
  /** Widen it for content beyond a question, such as fields or a list. */
  maxWidth?: DialogProps['maxWidth'];
  onClose: () => void;
  onConfirm: () => void;
  open?: boolean;
  slotProps?: DialogProps['slotProps'];
  title: ReactNode;
}) => {
  const { t } = useTranslation();
  return (
    <Dialog
      fullWidth
      maxWidth={maxWidth}
      onClose={isPending ? undefined : onClose}
      open={open}
      slotProps={slotProps}
    >
      <DialogTitle>{title}</DialogTitle>
      <DialogContent>
        <DialogContentText component="div">{children}</DialogContentText>
        <ApiErrorAlert error={error} sx={{ mt: 2 }} />
      </DialogContent>
      <DialogActions>
        <Button disabled={isPending} onClick={onClose}>
          {t('common.cancel')}
        </Button>
        <Button
          color={destructive ? 'error' : 'primary'}
          disabled={isPending || confirmDisabled}
          onClick={onConfirm}
          variant="contained"
        >
          {confirmLabel}
        </Button>
      </DialogActions>
    </Dialog>
  );
};
