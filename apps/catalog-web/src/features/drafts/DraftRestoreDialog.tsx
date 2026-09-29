import {
  Alert,
  Button,
  Dialog,
  DialogActions,
  DialogContent,
  DialogContentText,
  DialogTitle,
  Stack,
} from '@mui/material';
import { useId } from 'react';
import { useTranslation } from 'react-i18next';
import type { PendingDraft } from './useEditorDraft';

/**
 * Asks whether a saved draft should replace the loaded form. It cannot be
 * dismissed without a choice so the form never changes implicitly.
 */
export const DraftRestoreDialog = ({
  draft,
  onDiscard,
  onRestore,
}: {
  draft: PendingDraft<unknown> | undefined;
  onDiscard: () => void;
  onRestore: () => void;
}) => {
  const { i18n, t } = useTranslation();
  const titleId = useId();
  const descriptionId = useId();
  return (
    <Dialog
      aria-describedby={descriptionId}
      aria-labelledby={titleId}
      open={draft !== undefined}
    >
      <DialogTitle id={titleId}>{t('drafts.restoreTitle')}</DialogTitle>
      <DialogContent>
        <Stack spacing={2}>
          <DialogContentText id={descriptionId}>
            {t('drafts.restoreDescription', {
              savedAt: draft
                ? new Date(draft.savedAt).toLocaleString(i18n.language)
                : '',
            })}
          </DialogContentText>
          {draft?.sourceChanged && (
            <Alert severity="warning">{t('drafts.sourceChanged')}</Alert>
          )}
        </Stack>
      </DialogContent>
      <DialogActions>
        <Button color="error" onClick={onDiscard}>
          {t('drafts.discard')}
        </Button>
        <Button onClick={onRestore} variant="contained">
          {t('drafts.restore')}
        </Button>
      </DialogActions>
    </Dialog>
  );
};
