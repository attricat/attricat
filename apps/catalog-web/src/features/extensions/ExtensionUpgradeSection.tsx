import { Button, Paper, Stack, Typography } from '@mui/material';
import { useQuery } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import { CircleArrowUpIcon } from 'lucide-react';
import { ErrorNotice } from './ExtensionErrorNotice';
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

type ExtensionUpgradeSectionProps = {
  busy: boolean;
  canManage: boolean;
  installation: ExtensionInstallation;
  onUpgrade: (release: ExtensionReleaseReference) => void;
};

export const ExtensionUpgradeSection = ({
  busy,
  canManage,
  installation,
  onUpgrade,
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
      <ErrorNotice error={upgrades.error} />
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
    </Paper>
  );
};
