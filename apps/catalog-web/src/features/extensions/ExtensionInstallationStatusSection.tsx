import { Button, Chip, Paper, Stack, Typography } from '@mui/material';
import { useTranslation } from 'react-i18next';
import {
  PowerIcon,
  PowerOffIcon,
  ShieldAlertIcon,
  Trash2Icon,
} from 'lucide-react';
import {
  installationStateColor,
  installationStateLabelKey,
} from './extensionPageUtils';
import type {
  ExtensionInstallation,
  ExtensionLifecycleAction,
} from './managementApi';

type ExtensionInstallationStatusSectionProps = {
  busy: boolean;
  canManage: boolean;
  installation: ExtensionInstallation;
  onLifecycleAction: (action: ExtensionLifecycleAction) => void;
  onRemove: () => void;
};

export const ExtensionInstallationStatusSection = ({
  busy,
  canManage,
  installation,
  onLifecycleAction,
  onRemove,
}: ExtensionInstallationStatusSectionProps) => {
  const { t } = useTranslation();
  const locked = !canManage || busy;
  return (
    <Paper sx={{ p: 2 }}>
      <Stack direction="row" spacing={1} sx={{ alignItems: 'center' }}>
        <Chip
          color={installationStateColor(installation.state)}
          label={t(installationStateLabelKey(installation.state))}
        />
        <Typography>
          {t('extensions.manifestHash', {
            hash: installation.manifest_sha256,
          })}
        </Typography>
      </Stack>
      <Stack direction="row" spacing={1} sx={{ mt: 2 }}>
        <Button
          disabled={locked || installation.state === 'enabled'}
          onClick={() => onLifecycleAction('enable')}
          startIcon={<PowerIcon />}
        >
          {t('extensions.enable')}
        </Button>
        <Button
          disabled={locked || installation.state !== 'enabled'}
          onClick={() => onLifecycleAction('disable')}
          startIcon={<PowerOffIcon />}
        >
          {t('extensions.disable')}
        </Button>
        <Button
          color="warning"
          disabled={locked || installation.state === 'quarantined'}
          onClick={() => onLifecycleAction('quarantine')}
          startIcon={<ShieldAlertIcon />}
        >
          {t('extensions.quarantine')}
        </Button>
        <Button
          color="error"
          disabled={locked}
          onClick={onRemove}
          startIcon={<Trash2Icon />}
        >
          {t('extensions.remove')}
        </Button>
      </Stack>
    </Paper>
  );
};
