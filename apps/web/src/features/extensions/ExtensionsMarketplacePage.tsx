import { useQuery } from '@tanstack/react-query';
import { Link } from '@tanstack/react-router';
import { Box, Paper, Stack, Typography } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { useCan } from '../auth/useCan';
import { repositoryParts } from './extensionPageUtils';
import { discoverExtensions } from './managementApi';
import { extensionManagementQueryKeys } from './managementQueryKeys';
import { capabilities } from '../auth/capabilities';
import { ApiErrorAlert } from '../../components/ApiErrorAlert';
import { EmptyState } from '../../components/EmptyState';
import { LoadingState } from '../../components/LoadingState';
import { ExtensionIcon } from '../../components/systemIcons';

export const ExtensionsMarketplacePage = () => {
  const { t } = useTranslation();
  const canRead = useCan(capabilities.extensionsRead);
  const marketplace = useQuery({
    queryKey: extensionManagementQueryKeys.marketplace(),
    queryFn: discoverExtensions,
    enabled: canRead,
  });

  return (
    <>
      <ApiErrorAlert error={marketplace.error} sx={{ mb: 2 }} />
      <Typography component="h2" sx={{ mb: 1 }} variant="h5">
        {t('extensions.marketplace')}
      </Typography>
      {marketplace.isPending && canRead && (
        <LoadingState>{t('extensions.loadingExtensions')}</LoadingState>
      )}
      {marketplace.data?.length === 0 && (
        <Paper>
          <EmptyState
            icon={ExtensionIcon}
            title={t('extensions.noMarketplaceExtensions')}
          />
        </Paper>
      )}
      <Stack spacing={2}>
        {(marketplace.data ?? []).map((extension) => {
          const [owner, repository] = repositoryParts(extension.repository);
          return (
            <Paper
              key={`${extension.registry_source}:${extension.id}`}
              sx={{ p: 2 }}
            >
              <Box
                sx={{
                  display: 'flex',
                  justifyContent: 'space-between',
                  gap: 2,
                }}
              >
                <Box>
                  <Typography variant="h6">{extension.name}</Typography>
                  <Typography color="text.secondary">
                    {extension.description}
                  </Typography>
                  <Typography color="text.secondary" variant="caption">
                    {extension.registry_source}
                  </Typography>
                </Box>
                <Link
                  to="/manage/extensions/$owner/$repository"
                  params={{ owner, repository }}
                >
                  {t('extensions.inspect')}
                </Link>
              </Box>
            </Paper>
          );
        })}
      </Stack>
    </>
  );
};
