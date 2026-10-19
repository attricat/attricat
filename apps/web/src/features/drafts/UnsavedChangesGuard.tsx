import {
  Button,
  Dialog,
  DialogActions,
  DialogContent,
  DialogContentText,
  DialogTitle,
} from '@mui/material';
import { useBlocker, type ShouldBlockFn } from '@tanstack/react-router';
import { useId } from 'react';
import { useTranslation } from 'react-i18next';

const leavesPage: ShouldBlockFn = ({ current, next }) =>
  next.pathname !== current.pathname;

/**
 * Asks before an in-app navigation drops changes that are not saved. Render
 * it only while there are such changes; leaving the browser page is warned
 * about separately.
 */
export const UnsavedChangesGuard = ({
  title,
  description,
  shouldBlock = leavesPage,
  onDiscard,
}: {
  title: string;
  description: string;
  /** Which navigations leave the changes behind; by default, another page. */
  shouldBlock?: ShouldBlockFn;
  /** Runs when the user chooses to leave, before the navigation continues. */
  onDiscard?: () => void;
}) => {
  const { t } = useTranslation();
  const descriptionId = useId();
  const blocker = useBlocker({
    shouldBlockFn: shouldBlock,
    enableBeforeUnload: false,
    withResolver: true,
  });
  return (
    <Dialog
      aria-describedby={descriptionId}
      onClose={blocker.reset}
      open={blocker.status === 'blocked'}
    >
      <DialogTitle>{title}</DialogTitle>
      <DialogContent>
        <DialogContentText id={descriptionId}>{description}</DialogContentText>
      </DialogContent>
      <DialogActions>
        <Button onClick={blocker.reset}>{t('drafts.keepEditing')}</Button>
        <Button
          color="error"
          onClick={() => {
            onDiscard?.();
            blocker.proceed?.();
          }}
          variant="contained"
        >
          {t('drafts.discardAndLeave')}
        </Button>
      </DialogActions>
    </Dialog>
  );
};
