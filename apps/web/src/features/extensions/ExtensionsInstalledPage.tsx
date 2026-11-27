import { useQuery } from '@tanstack/react-query';
import { Box, Button, Paper, Stack, Typography } from '@mui/material';
import { CircleArrowUpIcon, StoreIcon } from 'lucide-react';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useCan } from '../auth/useCan';
import {
  installedExtensionName,
  marketplaceRepository,
} from './extensionPageUtils';
import { installedExtensions } from './managementApi';
import { extensionManagementQueryKeys } from './managementQueryKeys';
import { capabilities } from '../auth/capabilities';
import { ApiErrorAlert } from '../../components/ApiErrorAlert';
import { EmptyState } from '../../components/EmptyState';
import { LoadingState } from '../../components/LoadingState';
import { RouterButton, RouterLink } from '../../components/RouterLink';
import { ExtensionIcon } from '../../components/systemIcons';
import { InstallationStateChip } from './InstallationStateChip';
import { ExtensionUpdateChip } from './ExtensionUpdateChip';
import {
  ExtensionUpgradeDialog,
  type PendingExtensionUpgrade,
} from './ExtensionUpgradeDialog';
import { useExtensionUpdates } from './useExtensionUpdates';

export const ExtensionsInstalledPage = () => {
  const { t } = useTranslation();
  const canRead = useCan(capabilities.extensionsRead);
  const canManage = useCan(capabilities.extensionsManage);
  const installed = useQuery({
    queryKey: extensionManagementQueryKeys.installed(),
    queryFn: installedExtensions,
    enabled: canRead,
  });
  const updates = useExtensionUpdates();
  // Waiting for the person to confirm upgrading this extension.
  const [upgrade, setUpgrade] = useState<PendingExtensionUpgrade | null>(null);

  return (
    <>
      <ApiErrorAlert error={installed.error} sx={{ mb: 2 }} />
      <Typography component="h2" sx={{ mb: 1 }} variant="h5">
        {t('extensions.installed')}
      </Typography>
      {installed.isPending && canRead && (
        <LoadingState>{t('extensions.loadingExtensions')}</LoadingState>
      )}
      {installed.data?.length === 0 && (
        <Paper>
          <EmptyState
            action={
              <RouterButton
                startIcon={<StoreIcon />}
                to="/manage/extensions/marketplace"
                variant="contained"
              >
                {t('extensions.browseMarketplace')}
              </RouterButton>
            }
            description={t('extensions.noInstalledDescription')}
            icon={ExtensionIcon}
            title={t('extensions.noInstalled')}
          />
        </Paper>
      )}
      <Stack spacing={2}>
        {(installed.data ?? []).map((extension) => {
          const name = installedExtensionName(extension);
          const marketplace = marketplaceRepository(extension.source);
          const update = updates.find(
            (candidate) => candidate.extension_id === extension.extension_id,
          );
          return (
            <Paper key={extension.id} sx={{ p: 2 }}>
              <Box
                sx={{
                  alignItems: 'center',
                  display: 'flex',
                  flexWrap: 'wrap',
                  gap: 2,
                  justifyContent: 'space-between',
                }}
              >
                <Box>
                  <Typography component="h3" variant="h6">
                    {name} <InstallationStateChip state={extension.state} />{' '}
                    <ExtensionUpdateChip update={update} />
                  </Typography>
                  <Typography color="text.secondary">
                    {t('extensions.idVersionSource', {
                      id: extension.extension_id,
                      version: extension.version,
                      source: extension.source,
                    })}
                  </Typography>
                </Box>
                <Stack
                  direction="row"
                  spacing={2}
                  sx={{ alignItems: 'center', flexWrap: 'wrap' }}
                >
                  {marketplace && (
                    <RouterLink
                      params={marketplace}
                      to="/manage/extensions/$owner/$repository"
                    >
                      {t('extensions.viewInMarketplace')}
                    </RouterLink>
                  )}
                  <RouterLink
                    params={{ extensionId: extension.extension_id }}
                    to="/manage/extensions/$extensionId"
                  >
                    {t('extensions.manage')}
                  </RouterLink>
                  {update && marketplace && canManage && (
                    <Button
                      aria-label={t('extensions.updateExtension', { name })}
                      onClick={() =>
                        setUpgrade({
                          ...marketplace,
                          extensionId: extension.extension_id,
                          extensionName: name,
                          installedVersion: extension.version,
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
              </Box>
            </Paper>
          );
        })}
      </Stack>
      <ExtensionUpgradeDialog
        onClose={() => setUpgrade(null)}
        upgrade={upgrade}
      />
    </>
  );
};
