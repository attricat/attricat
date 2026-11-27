import {
  Alert,
  AlertTitle,
  Button,
  Divider,
  Stack,
  Typography,
} from '@mui/material';
import { useQuery } from '@tanstack/react-query';
import { Trans, useTranslation } from 'react-i18next';
import { CircleArrowUpIcon } from 'lucide-react';
import {
  isNewerRelease,
  marketplaceRepository,
  releaseTagVersion,
  sourceReleaseTag,
} from './extensionPageUtils';
import {
  registryDetails,
  type DiscoveredRelease,
  type ExtensionInstallation,
} from './managementApi';
import { extensionManagementQueryKeys } from './managementQueryKeys';
import { useExtensionUpdateList } from './useExtensionUpdates';
import { ApiErrorAlert } from '../../components/ApiErrorAlert';
import { Timestamp } from '../../time/Timestamp';

type ExtensionUpgradeSectionProps = {
  busy: boolean;
  canManage: boolean;
  installation: ExtensionInstallation;
  /** Asks to upgrade to this reviewed version; the caller confirms it. */
  onUpgrade: (version: string) => void;
};

const releaseName = (release: DiscoveredRelease) =>
  release.name || release.tag_name;

const Published = ({ release }: { release: DiscoveredRelease }) =>
  release.published_at ? (
    <Trans
      components={{
        timestamp: <Timestamp style="date" value={release.published_at} />,
      }}
      i18nKey="extensions.availableReleasesPublished"
    />
  ) : null;

/**
 * The registry's newer reviewed releases, led by the one to upgrade to;
 * nothing for a side-loaded extension or one that is up to date.
 */
export const ExtensionUpgradeSection = ({
  busy,
  canManage,
  installation,
  onUpgrade,
}: ExtensionUpgradeSectionProps) => {
  const { t } = useTranslation();
  const marketplace = marketplaceRepository(installation.source);
  const owner = marketplace?.owner ?? '';
  const repository = marketplace?.repository ?? '';
  const installedTag = sourceReleaseTag(installation.source);
  const upgrades = useQuery({
    queryKey: extensionManagementQueryKeys.registry(owner, repository),
    queryFn: () => registryDetails(owner, repository),
    enabled: marketplace !== undefined,
  });
  // Only the update list knows which versions this host can run: the
  // registry also lists releases for newer hosts. So nothing is offered
  // until it loads, then only its version and older releases.
  const updateList = useExtensionUpdateList();
  const reported = updateList.updates.find(
    (update) => update.extension_id === installation.extension_id,
  );
  const releases = (upgrades.data?.releases ?? []).filter(
    (release) =>
      release.tag_name !== installedTag &&
      isNewerRelease(release.tag_name, installation.version) &&
      reported !== undefined &&
      !isNewerRelease(release.tag_name, reported.latest_version),
  );
  if (upgrades.error) return <ApiErrorAlert error={upgrades.error} />;
  const latest = releases.find(
    (release) =>
      releaseTagVersion(release.tag_name) === reported?.latest_version,
  );
  if (!updateList.loaded || !latest) return null;
  const others = releases.filter((release) => release !== latest);
  const upgradeButton = (release: DiscoveredRelease, primary: boolean) => (
    <Button
      color={primary ? 'primary' : 'inherit'}
      disabled={!canManage || busy}
      onClick={() => onUpgrade(releaseTagVersion(release.tag_name))}
      startIcon={<CircleArrowUpIcon />}
      sx={{ flexShrink: 0 }}
      variant={primary ? 'contained' : 'text'}
    >
      {primary
        ? t('extensions.upgradeToVersion', {
            version: releaseTagVersion(release.tag_name),
          })
        : t('extensions.upgrade')}
    </Button>
  );
  return (
    <Alert component="section" icon={<CircleArrowUpIcon />} severity="info">
      <AlertTitle component="h2">
        {t('extensions.updateAvailable', {
          version: releaseTagVersion(latest.tag_name),
        })}
      </AlertTitle>
      <Stack
        direction={{ xs: 'column', sm: 'row' }}
        spacing={2}
        sx={{ alignItems: { sm: 'center' }, justifyContent: 'space-between' }}
      >
        <div>
          <Typography variant="body2">
            {t('extensions.upgradeFromTo', {
              from: installation.version,
              name: releaseName(latest),
            })}{' '}
            <Published release={latest} />
          </Typography>
          <Typography sx={{ mt: 1 }} variant="body2">
            {t('extensions.upgradeEffect')}
          </Typography>
        </div>
        {upgradeButton(latest, true)}
      </Stack>
      {others.length > 0 && (
        <>
          <Typography
            component="h3"
            sx={{ fontWeight: 'fontWeightBold', mt: 3, mb: 1 }}
            variant="body2"
          >
            {t('extensions.otherReleases')}
          </Typography>
          <Stack divider={<Divider />} spacing={1}>
            {others.map((release) => (
              <Stack
                direction="row"
                key={release.release_id}
                spacing={1}
                sx={{ alignItems: 'center', justifyContent: 'space-between' }}
              >
                <div>
                  <Typography variant="body2">
                    {releaseName(release)}
                  </Typography>
                  <Typography color="text.secondary" variant="caption">
                    <Published release={release} />
                  </Typography>
                </div>
                {upgradeButton(release, false)}
              </Stack>
            ))}
          </Stack>
        </>
      )}
    </Alert>
  );
};
