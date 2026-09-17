import { useForm } from '@tanstack/react-form';
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
import { useEffect, useRef } from 'react';
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
import { invalidateExtensions, repositoryParts } from './extension-page-utils';
import { workspaceExtensionLayoutSchema } from './schemas';

export const ExtensionsPage = () => {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
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
  const saveLayout = useMutation({
    mutationFn: updateWorkspaceExtensionLayout,
    onSuccess: () => {
      void queryClient.invalidateQueries({
        queryKey: extensionManagementQueryKeys.layout(),
      });
      invalidateExtensions(queryClient);
    },
  });
  const parseLayout = (draft: string) => {
    try {
      return workspaceExtensionLayoutSchema.safeParse(JSON.parse(draft));
    } catch {
      return undefined;
    }
  };
  const validateLayout = (draft: string) =>
    parseLayout(draft)?.success ? undefined : t('extensions.layoutInvalid');
  const form = useForm({
    defaultValues: { layout: '' },
    onSubmit: ({ value }) => {
      const parsed = parseLayout(value.layout);
      if (parsed?.success) saveLayout.mutate(parsed.data);
    },
  });
  const hydratedLayout = useRef<string | undefined>(undefined);
  useEffect(() => {
    if (layout.data) {
      const serialized = JSON.stringify(layout.data, null, 2);
      if (hydratedLayout.current !== serialized) {
        form.setFieldValue('layout', serialized);
        hydratedLayout.current = serialized;
      }
    }
  }, [form, layout.data]);
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
        <Paper
          component="form"
          onSubmit={(event) => {
            event.preventDefault();
            form.handleSubmit();
          }}
          sx={{ mb: 4, p: 2 }}
        >
          <Typography variant="h5">{t('extensions.layout')}</Typography>
          <Typography color="text.secondary" sx={{ mb: 2 }}>
            {t('extensions.layoutDescription')}
          </Typography>
          <form.Field
            name="layout"
            validators={{
              onBlur: ({ value }) => validateLayout(value),
              onChange: ({ value }) => validateLayout(value),
              onSubmit: ({ value }) => validateLayout(value),
            }}
          >
            {(field) => (
              <TextField
                error={field.state.meta.errors.length > 0}
                fullWidth
                helperText={field.state.meta.errors[0]}
                label={t('extensions.layout')}
                minRows={8}
                multiline
                onBlur={field.handleBlur}
                onChange={(event) => field.handleChange(event.target.value)}
                value={field.state.value}
              />
            )}
          </form.Field>
          <form.Subscribe
            selector={(state) => [state.canSubmit, state.values.layout]}
          >
            {([canSubmit, draft]) => (
              <Button
                disabled={!canSubmit || !draft || saveLayout.isPending}
                sx={{ mt: 2 }}
                type="submit"
              >
                {t('extensions.saveLayout')}
              </Button>
            )}
          </form.Subscribe>
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
