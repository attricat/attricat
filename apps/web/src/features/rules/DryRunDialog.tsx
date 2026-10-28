import {
  Button,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
} from '@mui/material';
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
  return (
    <Dialog
      fullWidth
      maxWidth="sm"
      onClose={onClose}
      open={dryRun !== undefined}
    >
      <DialogTitle>
        {dryRun && t('rules.dryRunResult.title', { name: dryRun.name })}
      </DialogTitle>
      <DialogContent>
        {dryRun && (
          <DryRunResult blueprintId={dryRun.blueprintId} runId={dryRun.runId} />
        )}
      </DialogContent>
      <DialogActions>
        <Button onClick={onClose}>{t('common.close')}</Button>
      </DialogActions>
    </Dialog>
  );
};
