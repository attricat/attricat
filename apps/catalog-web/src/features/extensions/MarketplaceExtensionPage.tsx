import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Link } from '@tanstack/react-router';
import { Alert, Button, Paper, Stack, Typography } from '@mui/material';
import ReactMarkdown from 'react-markdown';
import { useTranslation } from 'react-i18next';
import { PageContainer } from '../../components/PageContainer';
import { PageHeader } from '../../components/PageHeader';
import { currentSession } from '../auth/api';
import { authQueryKeys } from '../auth/query-keys';
import { installExtension, registryDetails } from './management-api';
import { extensionManagementQueryKeys } from './management-query-keys';
import { ErrorNotice } from './ExtensionErrorNotice';
import { invalidateExtensions } from './extension-page-utils';

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
  const canManage =
    useQuery({ queryKey: authQueryKeys.session(), queryFn: currentSession })
      .data?.capabilities?.extensions_manage === true;
  return (
    <PageContainer>
      <PageHeader
        title={
          details.data?.extension.name ?? t('extensions.extensionFallback')
        }
        description={
          details.data?.extension.description ?? t('extensions.loadingDetails')
        }
        actions={<Link to="/manage/extensions">{t('extensions.back')}</Link>}
      />
      <ErrorNotice error={details.error} />
      <ErrorNotice error={install.error} />
      {details.data && (
        <>
          <Paper sx={{ p: 2 }}>
            <Typography color="text.secondary">
              Origin: {details.data.extension.registry_source}
            </Typography>
            <Typography sx={{ mt: 2 }} variant="h6">
              Requested release
            </Typography>
            {details.data.releases.length === 0 ? (
              <Alert severity="info">
                No installable stable releases were found.
              </Alert>
            ) : (
              <Stack spacing={1} sx={{ mt: 1 }}>
                {details.data.releases.map((release) => (
                  <Stack
                    direction="row"
                    key={release.release_id}
                    spacing={2}
                    sx={{ alignItems: 'center' }}
                  >
                    <Typography>{release.name || release.tag_name}</Typography>
                    <Typography color="text.secondary" variant="body2">
                      {release.tag_name}
                    </Typography>
                    <Button
                      disabled={!canManage || install.isPending}
                      onClick={() =>
                        install.mutate({
                          owner,
                          repository,
                          release_id: release.release_id,
                        })
                      }
                      variant="contained"
                    >
                      Install
                    </Button>
                  </Stack>
                ))}
              </Stack>
            )}
          </Paper>
          <Paper sx={{ mt: 3, p: 2 }}>
            <Typography variant="h6">{t('extensions.readme')}</Typography>
            <ReactMarkdown>{details.data.readme}</ReactMarkdown>
          </Paper>
        </>
      )}
    </PageContainer>
  );
};
