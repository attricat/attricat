import { useQuery } from '@tanstack/react-query';
import { Link } from '@tanstack/react-router';
import { Box, Chip, Paper, Stack, Typography } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { useCan } from '../auth/useCan';
import {
  installationStateColor,
  installationStateLabelKey,
  installedExtensionName,
} from './extensionPageUtils';
import { installedExtensions } from './managementApi';
import { extensionManagementQueryKeys } from './managementQueryKeys';
import { capabilities } from '../auth/capabilities';
import { ApiErrorAlert } from '../../components/ApiErrorAlert';
import { EmptyState } from '../../components/EmptyState';
import { LoadingState } from '../../components/LoadingState';
import { ExtensionIcon } from '../../components/systemIcons';

export const ExtensionsInstalledPage = () => {
  const { t } = useTranslation();
  const canRead = useCan(capabilities.extensionsRead);
  const installed = useQuery({
    queryKey: extensionManagementQueryKeys.installed(),
    queryFn: installedExtensions,
    enabled: canRead,
  });

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
            description={t('extensions.noInstalledDescription')}
            icon={ExtensionIcon}
            title={t('extensions.noInstalled')}
          />
        </Paper>
      )}
      <Stack spacing={2}>
        {(installed.data ?? []).map((extension) => (
          <Paper key={extension.id} sx={{ p: 2 }}>
            <Box
              sx={{ display: 'flex', justifyContent: 'space-between', gap: 2 }}
            >
              <Box>
                <Typography component="h3" variant="h6">
                  {installedExtensionName(extension)}{' '}
                  <Chip
                    color={installationStateColor(extension.state)}
                    label={t(installationStateLabelKey(extension.state))}
                    size="small"
                  />
                </Typography>
                <Typography color="text.secondary">
                  {t('extensions.idVersionSource', {
                    id: extension.extension_id,
                    version: extension.version,
                    source: extension.source,
                  })}
                </Typography>
              </Box>
              <Link
                to="/manage/extensions/$extensionId"
                params={{ extensionId: extension.extension_id }}
              >
                {t('extensions.manage')}
              </Link>
            </Box>
          </Paper>
        ))}
      </Stack>
    </>
  );
};
