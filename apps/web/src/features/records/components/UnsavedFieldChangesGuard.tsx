import {
  Button,
  Dialog,
  DialogActions,
  DialogContent,
  DialogContentText,
  DialogTitle,
} from '@mui/material';
import { useBlocker } from '@tanstack/react-router';
import { useId } from 'react';
import { useTranslation } from 'react-i18next';

/** The record the Explorer shows in its panel, if the location has one. */
const panelRecord = (search: unknown) =>
  (search as { record?: unknown } | undefined)?.record;

/**
 * Asks before a navigation leaves the record while field changes that could
 * not be saved are still waiting. Render it only while there are such changes.
 */
export const UnsavedFieldChangesGuard = () => {
  const { t } = useTranslation();
  const descriptionId = useId();
  const blocker = useBlocker({
    shouldBlockFn: ({ current, next }) =>
      next.pathname !== current.pathname ||
      panelRecord(next.search) !== panelRecord(current.search),
    // The inline fields already warn before the page unloads.
    enableBeforeUnload: false,
    withResolver: true,
  });
  return (
    <Dialog
      aria-describedby={descriptionId}
      onClose={blocker.reset}
      open={blocker.status === 'blocked'}
    >
      <DialogTitle>{t('records.unsavedFieldChangesTitle')}</DialogTitle>
      <DialogContent>
        <DialogContentText id={descriptionId}>
          {t('records.unsavedFieldChangesDescription')}
        </DialogContentText>
      </DialogContent>
      <DialogActions>
        <Button onClick={blocker.reset}>{t('records.keepEditing')}</Button>
        <Button color="error" onClick={blocker.proceed} variant="contained">
          {t('records.discardAndLeave')}
        </Button>
      </DialogActions>
    </Dialog>
  );
};
