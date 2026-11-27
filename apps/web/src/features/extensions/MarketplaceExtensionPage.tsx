import { useState } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Link } from '@tanstack/react-router';
import {
  Alert,
  Box,
  Button,
  MenuItem,
  Paper,
  Stack,
  TextField,
  Typography,
} from '@mui/material';
import ReactMarkdown from 'react-markdown';
import { useTranslation } from 'react-i18next';
import { CircleArrowUpIcon, PackagePlusIcon } from 'lucide-react';
import { PageContainer } from '../../components/PageContainer';
import { PageHeader } from '../../components/PageHeader';
import { useCan } from '../auth/useCan';
import {
  installExtension,
  installedExtensions,
  registryDetails,
} from './managementApi';
import { extensionManagementQueryKeys } from './managementQueryKeys';
import {
  installedExtensionName,
  invalidateExtensions,
  sourceRepository,
} from './extensionPageUtils';
import {
  ExtensionUpgradeDialog,
  type PendingExtensionUpgrade,
} from './ExtensionUpgradeDialog';
import { useExtensionUpdates } from './useExtensionUpdates';
import { RouterButton } from '../../components/RouterLink';
import { ExtensionIcon } from '../../components/systemIcons';
import { capabilities } from '../auth/capabilities';
import { ApiErrorAlert } from '../../components/ApiErrorAlert';
import {
  githubMarkdownPlugins,
  githubMarkdownStyles,
} from '../markdown/githubMarkdown';

export const MarketplaceExtensionPage = ({
  owner,
  repository,
}: {
  owner: string;
  repository: string;
}) => {
  const { t } = useTranslation();
  const client = useQueryClient();
  const details = useQuery({
    queryKey: extensionManagementQueryKeys.registry(owner, repository),
    queryFn: () => registryDetails(owner, repository),
  });
  const install = useMutation({
    mutationFn: installExtension,
    onSuccess: () => invalidateExtensions(client),
  });
  const canManage = useCan(capabilities.extensionsManage);
  const installed = useQuery({
    queryKey: extensionManagementQueryKeys.installed(),
    queryFn: installedExtensions,
  });
  // Installing it again would fail: an installed extension is upgraded.
  const installation = installed.data?.find(
    (candidate) =>
      sourceRepository(candidate.source) === details.data?.extension.repository,
  );
  const update = useExtensionUpdates().find(
    (candidate) => candidate.extension_id === installation?.extension_id,
  );
  const [upgrade, setUpgrade] = useState<PendingExtensionUpgrade | null>(null);
  const [selectedReleaseId, setSelectedReleaseId] = useState<number>();
  const releases = details.data?.releases ?? [];
  const selectedRelease =
    releases.find((release) => release.release_id === selectedReleaseId) ??
    releases[0];
  return (
    <PageContainer>
      <PageHeader
        icon={ExtensionIcon}
        title={
          details.data?.extension.name ?? t('extensions.extensionFallback')
        }
        description={
          details.data?.extension.description ?? t('extensions.loadingDetails')
        }
        actions={
          <Link to="/manage/extensions/marketplace">
            {t('extensions.backToMarketplace')}
          </Link>
        }
      />
      <ApiErrorAlert error={details.error} sx={{ mb: 2 }} />
      <ApiErrorAlert error={install.error} sx={{ mb: 2 }} />
      {details.data && (
        <>
          <Paper sx={{ p: 2 }}>
            <Typography color="text.secondary">
              {t('extensions.origin', {
                origin: details.data.extension.registry_source,
              })}
            </Typography>
            {installation ? (
              <Alert
                action={
                  <Stack direction="row" spacing={1}>
                    <RouterButton
                      color="inherit"
                      params={{ extensionId: installation.extension_id }}
                      to="/manage/extensions/$extensionId"
                    >
                      {t('extensions.manage')}
                    </RouterButton>
                    {update && canManage && (
                      <Button
                        onClick={() =>
                          setUpgrade({
                            extensionId: installation.extension_id,
                            extensionName: installedExtensionName(installation),
                            installedVersion: installation.version,
                            owner,
                            repository,
                            version: update.latest_version,
                          })
                        }
                        startIcon={<CircleArrowUpIcon />}
                        variant="contained"
                      >
                        {t('extensions.update')}
                      </Button>
                    )}
                  </Stack>
                }
                severity={update ? 'info' : 'success'}
                sx={{ mt: 2 }}
              >
                {update
                  ? t('extensions.installedWithUpdate', {
                      installed: installation.version,
                      latest: update.latest_version,
                    })
                  : t('extensions.installedVersion', {
                      version: installation.version,
                    })}
              </Alert>
            ) : selectedRelease ? (
              <Stack
                direction={{ xs: 'column', sm: 'row' }}
                spacing={2}
                sx={{ alignItems: { sm: 'center' }, mt: 2 }}
              >
                <TextField
                  label={t('extensions.requestedRelease')}
                  onChange={(event) =>
                    setSelectedReleaseId(Number(event.target.value))
                  }
                  select
                  sx={{ flexGrow: 1 }}
                  value={selectedRelease.release_id}
                >
                  {releases.map((release) => (
                    <MenuItem
                      key={release.release_id}
                      value={release.release_id}
                    >
                      {release.name && release.name !== release.tag_name
                        ? `${release.name} (${release.tag_name})`
                        : release.tag_name}
                    </MenuItem>
                  ))}
                </TextField>
                <Button
                  disabled={!canManage || install.isPending}
                  onClick={() =>
                    install.mutate({
                      owner,
                      repository,
                      release_id: selectedRelease.release_id,
                    })
                  }
                  startIcon={<PackagePlusIcon />}
                  variant="contained"
                >
                  {t('extensions.install')}
                </Button>
              </Stack>
            ) : (
              <Alert severity="info" sx={{ mt: 2 }}>
                {t('extensions.noStableReleases')}
              </Alert>
            )}
          </Paper>
          <Paper sx={{ mt: 3, p: 2 }}>
            <Typography variant="h6">{t('extensions.readme')}</Typography>
            <Box sx={{ minWidth: 0, ...githubMarkdownStyles }}>
              <ReactMarkdown remarkPlugins={githubMarkdownPlugins}>
                {details.data.readme}
              </ReactMarkdown>
            </Box>
          </Paper>
        </>
      )}
      <ExtensionUpgradeDialog
        onClose={() => setUpgrade(null)}
        upgrade={upgrade}
      />
    </PageContainer>
  );
};
