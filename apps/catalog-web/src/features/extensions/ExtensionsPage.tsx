import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Link } from '@tanstack/react-router';
import {
  Alert,
  Box,
  Button,
  Chip,
  Paper,
  Stack,
  TextField,
  Typography,
} from '@mui/material';
import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { PageContainer } from '../../components/PageContainer';
import { PageHeader } from '../../components/PageHeader';
import { currentSession } from '../auth/api';
import { authQueryKeys } from '../auth/query-keys';
import {
  discoverExtensions,
  installedExtensions,
  updateWorkspaceExtensionLayout,
  workspaceExtensionLayout,
} from './management-api';
import { extensionManagementQueryKeys } from './management-query-keys';
import { ErrorNotice } from './ExtensionErrorNotice';
import { repositoryParts } from './extension-page-utils';
import { workspaceExtensionLayoutSchema } from './schemas';

export const ExtensionsPage = () => {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const [layoutDraft, setLayoutDraft] = useState('');
  const session = useQuery({
    queryKey: authQueryKeys.session(),
    queryFn: currentSession,
  });
  const marketplace = useQuery({
    queryKey: extensionManagementQueryKeys.marketplace(),
    queryFn: discoverExtensions,
    enabled: session.data?.capabilities?.extensions_read === true,
  });
  const installed = useQuery({
    queryKey: extensionManagementQueryKeys.installed(),
    queryFn: installedExtensions,
    enabled: session.data?.capabilities?.extensions_read === true,
  });
  const layout = useQuery({
    queryKey: extensionManagementQueryKeys.layout(),
    queryFn: workspaceExtensionLayout,
    enabled: session.data?.capabilities?.extensions_manage === true,
  });
  useEffect(() => {
    if (layout.data) setLayoutDraft(JSON.stringify(layout.data, null, 2));
  }, [layout.data]);
  const saveLayout = useMutation({
    mutationFn: (draft: string) =>
      updateWorkspaceExtensionLayout(
        workspaceExtensionLayoutSchema.parse(JSON.parse(draft)),
      ),
    onSuccess: () =>
      queryClient.invalidateQueries({
        queryKey: extensionManagementQueryKeys.layout(),
      }),
  });
  if (session.data && !session.data.capabilities?.extensions_read)
    return (
      <PageContainer>
        <Alert severity="error">{t('extensions.notAuthorizedView')}</Alert>
      </PageContainer>
    );
  return (
    <PageContainer>
      <PageHeader
        title={t('extensions.title')}
        description={t('extensions.description')}
        actions={
          session.data?.capabilities?.extensions_manage ? (
            <Button component={Link} to="/manage/extensions/sideload">
              {t('extensions.uploadArchive')}
            </Button>
          ) : undefined
        }
      />
      <ErrorNotice error={marketplace.error} />
      <ErrorNotice error={installed.error} />
      <ErrorNotice error={layout.error ?? saveLayout.error} />
      {session.data?.capabilities?.extensions_manage && (
        <Paper sx={{ mb: 4, p: 2 }}>
          <Typography variant="h5">{t('extensions.layout')}</Typography>
          <Typography color="text.secondary" sx={{ mb: 2 }}>
            {t('extensions.layoutDescription')}
          </Typography>
          <TextField
            fullWidth
            label={t('extensions.layout')}
            minRows={8}
            multiline
            onChange={(event) => setLayoutDraft(event.target.value)}
            value={layoutDraft}
          />
          <Button
            disabled={!layoutDraft || saveLayout.isPending}
            onClick={() => saveLayout.mutate(layoutDraft)}
            sx={{ mt: 2 }}
          >
            {t('extensions.saveLayout')}
          </Button>
        </Paper>
      )}
      <Typography sx={{ mb: 1 }} variant="h5">
        {t('extensions.marketplace')}
      </Typography>
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
      <Typography sx={{ mb: 1, mt: 4 }} variant="h5">
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
                    label={extension.state}
                    size="small"
                    color={
                      extension.state === 'enabled'
                        ? 'success'
                        : extension.state === 'quarantined'
                          ? 'error'
                          : 'default'
                    }
                  />
                </Typography>
                <Typography color="text.secondary">
                  v{extension.version} · {extension.source}
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
    </PageContainer>
  );
};
