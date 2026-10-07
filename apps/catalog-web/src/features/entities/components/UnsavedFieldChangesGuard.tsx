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

/** The entity the Explorer shows in its panel, if the location has one. */
const panelEntity = (search: unknown) =>
  (search as { entity?: unknown } | undefined)?.entity;

/**
 * Asks before a navigation leaves the entity while field changes that could
 * not be saved are still waiting. Render it only while there are such changes.
 */
export const UnsavedFieldChangesGuard = () => {
  const { t } = useTranslation();
  const descriptionId = useId();
  const blocker = useBlocker({
    shouldBlockFn: ({ current, next }) =>
      next.pathname !== current.pathname ||
      panelEntity(next.search) !== panelEntity(current.search),
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
      <DialogTitle>{t('entities.unsavedFieldChangesTitle')}</DialogTitle>
      <DialogContent>
        <DialogContentText id={descriptionId}>
          {t('entities.unsavedFieldChangesDescription')}
        </DialogContentText>
      </DialogContent>
      <DialogActions>
        <Button onClick={blocker.reset}>{t('entities.keepEditing')}</Button>
        <Button color="error" onClick={blocker.proceed} variant="contained">
          {t('entities.discardAndLeave')}
        </Button>
      </DialogActions>
    </Dialog>
  );
};
