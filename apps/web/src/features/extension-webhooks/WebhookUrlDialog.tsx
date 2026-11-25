import {
  Alert,
  Button,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  Stack,
  TextField,
  Typography,
} from '@mui/material';
import { CopyIcon } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { useCopyToClipboard } from '../../components/useCopyToClipboard';
import type { IssuedWebhookEndpoint } from './schemas';

/**
 * Shows the URL just issued for an endpoint. No later response carries it,
 * so only the Close button dismisses this; a stray click or Escape would
 * lose it.
 */
export const WebhookUrlDialog = ({
  issued,
  onClose,
}: {
  issued: IssuedWebhookEndpoint;
  onClose: () => void;
}) => {
  const { t } = useTranslation();
  const copyText = useCopyToClipboard();
  const value = issued.url ?? issued.path;
  return (
    <Dialog fullWidth maxWidth="sm" open>
      <DialogTitle>
        {t('extensionWebhooks.urlTitle', { name: issued.endpoint })}
      </DialogTitle>
      <DialogContent>
        <Stack spacing={2}>
          <Alert severity="warning">{t('extensionWebhooks.urlWarning')}</Alert>
          <TextField
            label={
              issued.url
                ? t('extensionWebhooks.url')
                : t('extensionWebhooks.path')
            }
            slotProps={{ input: { readOnly: true } }}
            value={value}
          />
          {!issued.url && (
            <Typography color="text.secondary" variant="body2">
              {t('extensionWebhooks.pathExplanation')}
            </Typography>
          )}
          <Typography color="text.secondary" variant="body2">
            {t('extensionWebhooks.urlInstructions')}
          </Typography>
          <Button
            onClick={() => void copyText(value)}
            startIcon={<CopyIcon />}
            variant="contained"
          >
            {issued.url
              ? t('extensionWebhooks.copyUrl')
              : t('extensionWebhooks.copyPath')}
          </Button>
        </Stack>
      </DialogContent>
      <DialogActions>
        <Button onClick={onClose}>{t('common.close')}</Button>
      </DialogActions>
    </Dialog>
  );
};
