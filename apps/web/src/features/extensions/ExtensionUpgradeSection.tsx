import { Button, Paper, Stack, Typography } from '@mui/material';
import { useQuery } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import { CircleArrowUpIcon, UploadIcon } from 'lucide-react';
import { extensionArchiveAccept } from './constants';
import {
  repositoryParts,
  sourceReleaseTag,
  sourceRepository,
} from './extensionPageUtils';
import {
  registryDetails,
  type ExtensionInstallation,
  type ExtensionReleaseReference,
} from './managementApi';
import { extensionManagementQueryKeys } from './managementQueryKeys';
import { ApiErrorAlert } from '../../components/ApiErrorAlert';

type ExtensionUpgradeSectionProps = {
  busy: boolean;
  canManage: boolean;
  installation: ExtensionInstallation;
  onUpgrade: (release: ExtensionReleaseReference) => void;
  onUpgradeArchive: (archive: File) => void;
};

export const ExtensionUpgradeSection = ({
  busy,
  canManage,
  installation,
  onUpgrade,
  onUpgradeArchive,
}: ExtensionUpgradeSectionProps) => {
  const { t } = useTranslation();
  const [owner = '', repository = ''] = repositoryParts(
    sourceRepository(installation.source),
  );
  const installedTag = sourceReleaseTag(installation.source);
  const upgrades = useQuery({
    queryKey: extensionManagementQueryKeys.registry(owner, repository),
    queryFn: () => registryDetails(owner, repository),
    enabled: Boolean(owner && repository),
  });
  const releases = (upgrades.data?.releases ?? []).filter(
    (release) => release.tag_name !== installedTag,
  );
  return (
    <Paper sx={{ p: 2 }}>
      <Typography variant="h6">{t('extensions.upgrade')}</Typography>
      <ApiErrorAlert error={upgrades.error} sx={{ mb: 2 }} />
      {releases.map((release) => (
        <Stack
          direction="row"
          key={release.release_id}
          spacing={1}
          sx={{ alignItems: 'center' }}
        >
          <Typography>{release.name || release.tag_name}</Typography>
          <Button
            disabled={!canManage || busy}
            onClick={() =>
              onUpgrade({
                owner,
                repository,
                release_id: release.release_id,
              })
            }
            startIcon={<CircleArrowUpIcon />}
          >
            {t('extensions.upgrade')}
          </Button>
        </Stack>
      ))}
      <Stack spacing={1} sx={{ alignItems: 'flex-start', mt: 1 }}>
        <Typography color="text.secondary" variant="body2">
          {t('extensions.upgradeArchiveHint')}
        </Typography>
        <Button
          component="label"
          disabled={!canManage || busy}
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
              if (archive) onUpgradeArchive(archive);
            }}
            type="file"
          />
        </Button>
      </Stack>
    </Paper>
  );
};
