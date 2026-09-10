import { useForm } from '@tanstack/react-form';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Link } from '@tanstack/react-router';
import {
  Alert,
  Box,
  Button,
  Chip,
  Divider,
  Paper,
  Stack,
  TextField,
  Typography,
} from '@mui/material';
import { useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { PageContainer } from '../../components/PageContainer';
import { PageHeader } from '../../components/PageHeader';
import { currentSession } from '../auth/api';
import { authQueryKeys } from '../auth/query-keys';
import {
  configureExtension,
  extensionDetail,
  lifecycleExtension,
  registryDetails,
  removeExtension,
  upgradeExtension,
} from './management-api';
import { extensionManagementQueryKeys } from './management-query-keys';
import { ErrorNotice } from './ExtensionErrorNotice';
import { invalidateExtensions, repositoryParts } from './extension-page-utils';
import { ExtensionPermissionsSection } from './ExtensionPermissionsSection';

export const InstalledExtensionPage = ({
  extensionId,
}: {
  extensionId: string;
}) => {
  const { t } = useTranslation();
  const client = useQueryClient();
  const detail = useQuery({
    queryKey: extensionManagementQueryKeys.detail(extensionId),
    queryFn: () => extensionDetail(extensionId),
  });
  const manage =
    useQuery({ queryKey: authQueryKeys.session(), queryFn: currentSession })
      .data?.capabilities?.extensions_manage === true;
  const action = useMutation({
    mutationFn: ({
      action,
      release,
    }: {
      action: 'enable' | 'disable' | 'quarantine' | 'remove' | 'upgrade';
      release?: { owner: string; repository: string; release_id: number };
    }) =>
      (action === 'remove'
        ? removeExtension(extensionId)
        : action === 'upgrade'
          ? upgradeExtension(extensionId, release!)
          : lifecycleExtension(extensionId, action)
      ).then(() => undefined),
    onSuccess: () => invalidateExtensions(client, extensionId),
  });
  const config = useMutation({
    mutationFn: (value: unknown) => configureExtension(extensionId, value),
    onSuccess: () => invalidateExtensions(client, extensionId),
  });
  const [configurationError, setConfigurationError] = useState<string>();
  const hydratedRelease = useRef<string | undefined>(undefined);
  const form = useForm({
    defaultValues: { configuration: '{}' },
    onSubmit: ({ value }) => {
      try {
        setConfigurationError(undefined);
        config.mutate(JSON.parse(value.configuration));
      } catch {
        setConfigurationError(t('extensions.configurationJsonError'));
      }
    },
  });
  const installation = detail.data?.installation;
  useEffect(() => {
    if (
      installation &&
      hydratedRelease.current !== installation.installed_release_id
    ) {
      form.setFieldValue(
        'configuration',
        JSON.stringify(installation.configuration, null, 2),
      );
      hydratedRelease.current = installation.installed_release_id;
    }
  }, [form, installation]);
  const [upgradeOwner = '', upgradeRepository = ''] = installation
    ? repositoryParts(installation.source.replace(/@.*$/, ''))
    : [];
  const upgrades = useQuery({
    queryKey: extensionManagementQueryKeys.registry(
      upgradeOwner,
      upgradeRepository,
    ),
    queryFn: () => registryDetails(upgradeOwner, upgradeRepository),
    enabled: Boolean(upgradeOwner && upgradeRepository),
  });
  return (
    <PageContainer>
      <PageHeader
        title={installation?.extension_id ?? t('extensions.extensionFallback')}
        description={
          installation
            ? t('extensions.installationVersion', {
                version: installation.version,
                source: installation.source,
              })
            : t('extensions.loadingInstallation')
        }
        actions={<Link to="/manage/extensions">{t('extensions.back')}</Link>}
      />
      <ErrorNotice error={detail.error} />
      <ErrorNotice error={action.error} />
      <ErrorNotice error={config.error} />
      {configurationError && (
        <Alert severity="error">{configurationError}</Alert>
      )}
      {detail.data && (
        <Stack spacing={3}>
          <Paper sx={{ p: 2 }}>
            <Stack direction="row" spacing={1} sx={{ alignItems: 'center' }}>
              <Chip
                label={installation!.state}
                color={
                  installation!.state === 'enabled'
                    ? 'success'
                    : installation!.state === 'quarantined'
                      ? 'error'
                      : 'default'
                }
              />
              <Typography>
                {t('extensions.manifestHash', {
                  hash: installation!.manifest_sha256,
                })}
              </Typography>
            </Stack>
            <Stack direction="row" spacing={1} sx={{ mt: 2 }}>
              <Button
                disabled={
                  !manage ||
                  action.isPending ||
                  installation!.state === 'enabled'
                }
                onClick={() => action.mutate({ action: 'enable' })}
              >
                {t('extensions.enable')}
              </Button>
              <Button
                disabled={
                  !manage ||
                  action.isPending ||
                  installation!.state !== 'enabled'
                }
                onClick={() => action.mutate({ action: 'disable' })}
              >
                {t('extensions.disable')}
              </Button>
              <Button
                color="warning"
                disabled={
                  !manage ||
                  action.isPending ||
                  installation!.state === 'quarantined'
                }
                onClick={() => action.mutate({ action: 'quarantine' })}
              >
                {t('extensions.quarantine')}
              </Button>
              <Button
                color="error"
                disabled={!manage || action.isPending}
                onClick={() => action.mutate({ action: 'remove' })}
              >
                {t('extensions.remove')}
              </Button>
            </Stack>
          </Paper>
          <Paper sx={{ p: 2 }}>
            <Typography variant="h6">{t('extensions.upgrade')}</Typography>
            <ErrorNotice error={upgrades.error} />
            {(upgrades.data?.releases ?? [])
              .filter(
                (release) =>
                  release.tag_name !== installation!.source.split('@').at(-1),
              )
              .map((release) => (
                <Stack
                  direction="row"
                  key={release.release_id}
                  spacing={1}
                  sx={{ alignItems: 'center' }}
                >
                  <Typography>{release.name || release.tag_name}</Typography>
                  <Button
                    disabled={!manage || action.isPending}
                    onClick={() =>
                      action.mutate({
                        action: 'upgrade',
                        release: {
                          owner: upgradeOwner,
                          repository: upgradeRepository,
                          release_id: release.release_id,
                        },
                      })
                    }
                  >
                    {t('extensions.upgrade')}
                  </Button>
                </Stack>
              ))}
          </Paper>
          <Paper sx={{ p: 2 }}>
            <Typography variant="h6">
              {t('extensions.configuration')}
            </Typography>
            <Typography color="text.secondary" variant="body2">
              {t('extensions.serverValidation')}
            </Typography>
            <form
              onSubmit={(event) => {
                event.preventDefault();
                void form.handleSubmit();
              }}
            >
              <form.Field name="configuration">
                {(field) => (
                  <TextField
                    fullWidth
                    label={t('extensions.jsonConfiguration')}
                    multiline
                    onChange={(event) => field.handleChange(event.target.value)}
                    sx={{ mt: 2 }}
                    value={field.state.value}
                    minRows={5}
                  />
                )}
              </form.Field>
              <Button
                disabled={!manage || config.isPending}
                sx={{ mt: 1 }}
                type="submit"
                variant="contained"
              >
                {t('extensions.saveConfiguration')}
              </Button>
            </form>
          </Paper>
          <ExtensionPermissionsSection
            enabled={manage}
            extensionId={extensionId}
            grants={detail.data.grants}
            manifest={installation!.manifest}
            onChanged={() => invalidateExtensions(client, extensionId)}
          />
          <Paper sx={{ p: 2 }}>
            <Typography variant="h6">{t('extensions.lifecycle')}</Typography>
            {detail.data.lifecycle.length === 0 ? (
              <Typography color="text.secondary">
                {t('extensions.noLifecycle')}
              </Typography>
            ) : (
              detail.data.lifecycle.map((item) => (
                <Box key={item.id} sx={{ py: 1 }}>
                  <Typography>
                    {item.operation}: {item.prior_state ?? '—'} →{' '}
                    {item.new_state ?? '—'}
                  </Typography>
                  <Typography color="text.secondary" variant="body2">
                    {new Date(item.created_at).toLocaleString()}{' '}
                    {item.actor_user_id
                      ? t('extensions.lifecycleActor', {
                          actor: item.actor_user_id,
                        })
                      : ''}{' '}
                    {Object.keys(item.diagnostics as object).length
                      ? `· ${JSON.stringify(item.diagnostics)}`
                      : ''}
                  </Typography>
                  <Divider />
                </Box>
              ))
            )}
          </Paper>
        </Stack>
      )}
    </PageContainer>
  );
};
