import {
  Button,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
} from '@mui/material';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { DryRunResult } from './DryRunResult';
import type { ShownDryRun } from './useRunRule';

/** The result of a dry run started from a list or a rule's page. */
export const DryRunDialog = ({
  dryRun,
  onClose,
}: {
  dryRun: ShownDryRun | undefined;
  onClose: () => void;
}) => {
  const { t } = useTranslation();
  // The closing dialog keeps showing its dry run until it has faded out.
  const [shown, setShown] = useState(dryRun);
  if (dryRun !== undefined && dryRun !== shown) setShown(dryRun);
  return (
    <Dialog
      fullWidth
      maxWidth="sm"
      onClose={onClose}
      open={dryRun !== undefined}
      slotProps={{ transition: { onExited: () => setShown(undefined) } }}
    >
      <DialogTitle>
        {shown && t('rules.dryRunResult.title', { name: shown.name })}
      </DialogTitle>
      <DialogContent>
        {shown && (
          <DryRunResult blueprintId={shown.blueprintId} runId={shown.runId} />
        )}
      </DialogContent>
      <DialogActions>
        <Button onClick={onClose}>{t('common.close')}</Button>
      </DialogActions>
    </Dialog>
  );
};
