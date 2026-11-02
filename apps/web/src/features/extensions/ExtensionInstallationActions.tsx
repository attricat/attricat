import { Button, Stack } from '@mui/material';
import { useTranslation } from 'react-i18next';
import {
  PowerIcon,
  PowerOffIcon,
  ShieldAlertIcon,
  Trash2Icon,
  UploadIcon,
} from 'lucide-react';
import { extensionArchiveAccept, installationStates } from './constants';
import type {
  ExtensionInstallation,
  ExtensionLifecycleAction,
} from './managementApi';

type ExtensionInstallationActionsProps = {
  busy: boolean;
  installation: ExtensionInstallation;
  onArchiveChosen: (archive: File) => void;
  onLifecycleAction: (action: ExtensionLifecycleAction) => void;
  onRemove: () => void;
};

/** What an operator can do to an installation, from any tab of its page. */
export const ExtensionInstallationActions = ({
  busy,
  installation,
  onArchiveChosen,
  onLifecycleAction,
  onRemove,
}: ExtensionInstallationActionsProps) => {
  const { t } = useTranslation();
  const enabled = installation.state === installationStates.enabled;
  return (
    <Stack
      direction="row"
      sx={{
        flexWrap: { xs: 'wrap', lg: 'nowrap' },
        whiteSpace: 'nowrap',
        gap: 1,
        justifyContent: { sm: 'flex-end' },
      }}
    >
      <Button
        disabled={busy}
        onClick={() => onLifecycleAction(enabled ? 'disable' : 'enable')}
        startIcon={enabled ? <PowerOffIcon /> : <PowerIcon />}
        variant="outlined"
      >
        {enabled ? t('extensions.disable') : t('extensions.enable')}
      </Button>
      <Button
        component="label"
        disabled={busy}
        startIcon={<UploadIcon />}
        variant="outlined"
      >
        {t('extensions.upgradeFromArchive')}
        <input
          accept={extensionArchiveAccept}
          hidden
          onChange={(event) => {
            const archive = event.target.files?.[0];
            event.target.value = '';
            if (archive) onArchiveChosen(archive);
          }}
          type="file"
        />
      </Button>
      <Button
        color="warning"
        disabled={busy || installation.state === installationStates.quarantined}
        onClick={() => onLifecycleAction('quarantine')}
        startIcon={<ShieldAlertIcon />}
        variant="outlined"
      >
        {t('extensions.quarantine')}
      </Button>
      <Button
        color="error"
        disabled={busy}
        onClick={onRemove}
        startIcon={<Trash2Icon />}
        variant="outlined"
      >
        {t('extensions.remove')}
      </Button>
    </Stack>
  );
};
