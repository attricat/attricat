import {
  Alert,
  Button,
  Dialog,
  DialogContent,
  DialogTitle,
  Stack,
  TextField,
} from '@mui/material';
import { useTranslation } from 'react-i18next';
import { copyToClipboard } from '../../components/clipboard';
import { useToast } from '../../components/useToast';
import { secretDialogMinWidth } from './constants';

export const SecretDialog = ({
  secret,
  onClose,
}: {
  secret?: string;
  onClose: () => void;
}) => {
  const { t } = useTranslation();
  const { show } = useToast();
  const copySecret = async () => {
    try {
      await copyToClipboard(secret ?? '');
      show({ message: t('common.copied'), severity: 'success' });
    } catch {
      show({ message: t('common.copyFailed'), severity: 'error' });
    }
  };

  return (
    <Dialog onClose={onClose} open={Boolean(secret)}>
      <DialogTitle>{t('profile.copySecretTitle')}</DialogTitle>
      <DialogContent>
        <Stack spacing={2} sx={{ minWidth: secretDialogMinWidth }}>
          <Alert severity="warning">{t('profile.copySecretWarning')}</Alert>
          <TextField
            slotProps={{ input: { readOnly: true } }}
            value={secret ?? ''}
          />
          <Button onClick={() => void copySecret()} variant="contained">
            {t('profile.copySecret')}
          </Button>
        </Stack>
      </DialogContent>
    </Dialog>
  );
};
