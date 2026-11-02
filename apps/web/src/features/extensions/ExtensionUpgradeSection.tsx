import { Button, Divider, Paper, Stack, Typography } from '@mui/material';
import { useQuery } from '@tanstack/react-query';
import { Trans, useTranslation } from 'react-i18next';
import { CircleArrowUpIcon } from 'lucide-react';
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
import { Timestamp } from '../../time/Timestamp';

type ExtensionUpgradeSectionProps = {
  busy: boolean;
  canManage: boolean;
  installation: ExtensionInstallation;
  onUpgrade: (release: ExtensionReleaseReference) => void;
};

/** The registry's other releases; nothing for a side-loaded extension. */
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
  if (!upgrades.error && releases.length === 0) return null;
  return (
    <Paper sx={{ p: 4 }}>
      <Typography component="h2" variant="h6">
        {t('extensions.availableReleases')}
      </Typography>
      <Typography color="text.secondary" sx={{ mb: 2 }} variant="body2">
        {t('extensions.upgradeEffect')}
      </Typography>
      <ApiErrorAlert error={upgrades.error} sx={{ mb: 2 }} />
      <Stack divider={<Divider />} spacing={1}>
        {releases.map((release) => (
          <Stack
            direction="row"
            key={release.release_id}
            spacing={1}
            sx={{ alignItems: 'center', justifyContent: 'space-between' }}
          >
            <div>
              <Typography>{release.name || release.tag_name}</Typography>
              {release.published_at && (
                <Typography color="text.secondary" variant="body2">
                  <Trans
                    components={{
                      timestamp: (
                        <Timestamp style="date" value={release.published_at} />
                      ),
                    }}
                    i18nKey="extensions.availableReleasesPublished"
                    t={t}
                  />
                </Typography>
              )}
            </div>
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
      </Stack>
    </Paper>
  );
};
