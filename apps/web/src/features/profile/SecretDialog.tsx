import {
  Alert,
  Button,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  Stack,
  TextField,
} from '@mui/material';
import { useTranslation } from 'react-i18next';
import { CopyIcon } from 'lucide-react';
import { useCopyToClipboard } from '../../components/useCopyToClipboard';

export const SecretDialog = ({
  secret,
  onClose,
}: {
  secret?: string;
  onClose: () => void;
}) => {
  const { t } = useTranslation();
  const copyText = useCopyToClipboard();
  const copySecret = () => copyText(secret ?? '');

  return (
    <Dialog fullWidth maxWidth="xs" onClose={onClose} open={Boolean(secret)}>
      <DialogTitle>{t('profile.copySecretTitle')}</DialogTitle>
      <DialogContent>
        <Stack spacing={2}>
          <Alert severity="warning">{t('profile.copySecretWarning')}</Alert>
          <TextField
            slotProps={{ input: { readOnly: true } }}
            value={secret ?? ''}
          />
          <Button
            onClick={() => void copySecret()}
            startIcon={<CopyIcon />}
            variant="contained"
          >
            {t('profile.copySecret')}
          </Button>
        </Stack>
      </DialogContent>
      <DialogActions>
        <Button onClick={onClose}>{t('common.close')}</Button>
      </DialogActions>
    </Dialog>
  );
};
