import { useMutation, useQueryClient } from '@tanstack/react-query';
import {
  Button,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  MenuItem,
  TextField,
  Typography,
} from '@mui/material';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { ApiErrorAlert } from '../../../components/ApiErrorAlert';
import { unlockRecordStatus } from '../api';
import { invalidateRecord } from '../invalidateRecord';
import { attributeLabel } from '../recordDisplay';
import { statusOptionLabel, type StatusLock } from '../status';

/**
 * Lets an owner or administrator move a locking status to any option, outside
 * the blueprint's transitions. The server audits it as an unlock.
 */
export const UnlockStatusDialog = ({
  lock: { attribute, config, option },
  recordId,
  contextId,
  updatedAt,
  suggested,
  onClose,
}: {
  lock: StatusLock;
  recordId: string;
  contextId: string | null;
  updatedAt: string;
  /** The option to preselect, such as a declared correction target. */
  suggested?: string;
  onClose: () => void;
}) => {
  const { t } = useTranslation();
  const client = useQueryClient();
  const targets = config.options.filter((item) => item.code !== option.code);
  const [to, setTo] = useState(
    targets.some((item) => item.code === suggested)
      ? suggested
      : targets[0]?.code,
  );
  const unlock = useMutation({
    mutationFn: (target: string) =>
      unlockRecordStatus(recordId, {
        attributeCode: attribute.code,
        contextId,
        to: target,
        expectedUpdatedAt: updatedAt,
      }),
    onSuccess: async () => {
      await invalidateRecord(client, recordId);
      onClose();
    },
  });
  const status = attributeLabel(attribute);

  return (
    <Dialog
      open
      onClose={unlock.isPending ? undefined : onClose}
      fullWidth
      maxWidth="xs"
    >
      <DialogTitle>{t('records.statusLock.unlockTitle')}</DialogTitle>
      <DialogContent>
        <Typography sx={{ mb: 3 }}>
          {t('records.statusLock.unlockExplanation', {
            attribute: status,
            status: statusOptionLabel(option),
          })}
        </Typography>
        <TextField
          disabled={unlock.isPending}
          fullWidth
          label={t('records.statusLock.unlockTo', { attribute: status })}
          onChange={(event) => setTo(event.target.value)}
          select
          value={to ?? ''}
        >
          {targets.map((item) => (
            <MenuItem key={item.code} value={item.code}>
              {statusOptionLabel(item)}
            </MenuItem>
          ))}
        </TextField>
        <ApiErrorAlert error={unlock.error} sx={{ mt: 2 }} />
      </DialogContent>
      <DialogActions>
        <Button disabled={unlock.isPending} onClick={onClose}>
          {t('common.cancel')}
        </Button>
        <Button
          disabled={unlock.isPending || to === undefined}
          onClick={() => to && unlock.mutate(to)}
          variant="contained"
        >
          {t('records.statusLock.unlock')}
        </Button>
      </DialogActions>
    </Dialog>
  );
};
