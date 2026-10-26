import {
  Box,
  Button,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  type DialogProps,
} from '@mui/material';
import type { ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { ApiErrorAlert } from './ApiErrorAlert';

/**
 * A dialog around a form that cannot be closed while it saves; a failed save
 * is shown below the fields so the user can correct them and try again.
 */
export const FormDialog = ({
  children,
  error,
  isPending,
  onClose,
  onSubmit,
  slotProps,
  submitDisabled = false,
  submitLabel,
  title,
}: {
  children: ReactNode;
  error?: Error | null;
  isPending: boolean;
  onClose: () => void;
  onSubmit: () => void;
  slotProps?: DialogProps['slotProps'];
  submitDisabled?: boolean;
  submitLabel: ReactNode;
  title: ReactNode;
}) => {
  const { t } = useTranslation();
  return (
    <Dialog
      fullWidth
      maxWidth="sm"
      onClose={isPending ? undefined : onClose}
      open
      slotProps={slotProps}
    >
      <Box
        component="form"
        onSubmit={(event) => {
          event.preventDefault();
          onSubmit();
        }}
      >
        <DialogTitle>{title}</DialogTitle>
        <DialogContent>
          {children}
          <ApiErrorAlert error={error} sx={{ mt: 2 }} />
        </DialogContent>
        <DialogActions>
          <Button disabled={isPending} onClick={onClose}>
            {t('common.cancel')}
          </Button>
          <Button
            disabled={isPending || submitDisabled}
            type="submit"
            variant="contained"
          >
            {submitLabel}
          </Button>
        </DialogActions>
      </Box>
    </Dialog>
  );
};
