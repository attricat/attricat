import { useQuery } from '@tanstack/react-query';
import { Link } from '@tanstack/react-router';
import { Box, Chip, Paper, Stack, Typography } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { currentSession } from '../auth/api';
import { authQueryKeys } from '../auth/queryKeys';
import { ErrorNotice } from './ExtensionErrorNotice';
import {
  installationStateColor,
  installationStateLabelKey,
} from './extensionPageUtils';
import { installedExtensions } from './managementApi';
import { extensionManagementQueryKeys } from './managementQueryKeys';

export const ExtensionsInstalledPage = () => {
  const { t } = useTranslation();
  const session = useQuery({
    queryKey: authQueryKeys.session(),
    queryFn: currentSession,
  });
  const installed = useQuery({
    queryKey: extensionManagementQueryKeys.installed(),
    queryFn: installedExtensions,
    enabled: session.data?.capabilities?.extensions_read === true,
  });

  return (
    <>
      <ErrorNotice error={installed.error} />
      <Typography sx={{ mb: 1 }} variant="h5">
        {t('extensions.installed')}
      </Typography>
      <Stack spacing={2}>
        {(installed.data ?? []).map((extension) => (
          <Paper key={extension.id} sx={{ p: 2 }}>
            <Box
              sx={{ display: 'flex', justifyContent: 'space-between', gap: 2 }}
            >
              <Box>
                <Typography variant="h6">
                  {extension.extension_id}{' '}
                  <Chip
                    color={installationStateColor(extension.state)}
                    label={t(installationStateLabelKey(extension.state))}
                    size="small"
                  />
                </Typography>
                <Typography color="text.secondary">
                  {t('extensions.versionSource', {
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
