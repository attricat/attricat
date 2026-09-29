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
import { Trans, useTranslation } from 'react-i18next';
import { Timestamp } from '../../time/Timestamp';
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
  const { t } = useTranslation();
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
            <Trans
              components={{ timestamp: <Timestamp value={draft?.savedAt} /> }}
              i18nKey="drafts.restoreDescription"
              t={t}
            />
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
