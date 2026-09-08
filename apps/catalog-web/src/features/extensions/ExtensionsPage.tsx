import { useForm } from '@tanstack/react-form';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useEffect, useRef, useState } from 'react';
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
import ReactMarkdown from 'react-markdown';
import { useTranslation } from 'react-i18next';
import { PageContainer } from '../../components/PageContainer';
import { PageHeader } from '../../components/PageHeader';
import { currentSession } from '../auth/api';
import { authQueryKeys } from '../auth/query-keys';
import {
  discoverExtensions,
  extensionDetail,
  grantExtension,
  installExtension,
  installedExtensions,
  lifecycleExtension,
  registryDetails,
  removeExtension,
  revokeExtensionGrant,
  configureExtension,
  sideloadExtension,
  upgradeExtension,
} from './management-api';
import { extensionManagementQueryKeys } from './management-query-keys';
import { extensionQueryKeys } from './query-keys';

const repositoryParts = (repository: string) =>
  repository.replace(/^github:/, '').split('/', 2);
const ErrorNotice = ({ error }: { error: Error | null }) =>
  error ? (
    <Alert severity="error" sx={{ mb: 2 }}>
      {error.message}
    </Alert>
  ) : null;
const invalidate = (
  client: ReturnType<typeof useQueryClient>,
  extensionId?: string,
) => {
  void client.invalidateQueries({ queryKey: extensionManagementQueryKeys.all });
  void client.invalidateQueries({ queryKey: extensionQueryKeys.runtime() });
  if (extensionId)
    void client.invalidateQueries({
      queryKey: extensionManagementQueryKeys.detail(extensionId),
    });
};

export const ExtensionsPage = () => {
  const { t } = useTranslation();
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

export const SideloadExtensionPage = () => {
  const { t } = useTranslation();
  const client = useQueryClient();
  const session = useQuery({
    queryKey: authQueryKeys.session(),
    queryFn: currentSession,
  });
  const sideload = useMutation({
    mutationFn: sideloadExtension,
    onSuccess: () => invalidate(client),
  });
  const [archive, setArchive] = useState<File | null>(null);
  const form = useForm({
    defaultValues: { archive: null as File | null },
    onSubmit: ({ value }) => {
      if (value.archive) sideload.mutate(value.archive);
    },
  });
  const canManage = session.data?.capabilities?.extensions_manage === true;
  if (session.data && !canManage)
    return (
      <PageContainer>
        <Alert severity="error">{t('extensions.notAuthorizedInstall')}</Alert>
      </PageContainer>
    );
  return (
    <PageContainer>
      <PageHeader
        title={t('extensions.uploadTitle')}
        description={t('extensions.uploadDescription')}
        actions={<Link to="/manage/extensions">Back to extensions</Link>}
      />
      <ErrorNotice error={sideload.error} />
      {sideload.isSuccess && (
        <Alert severity="success" sx={{ mb: 2 }}>
          {t('extensions.installedSuccess')}
        </Alert>
      )}
      <Paper sx={{ p: 2 }}>
        <form
          onSubmit={(event) => {
            event.preventDefault();
            void form.handleSubmit();
          }}
        >
          <form.Field name="archive">
            {(field) => (
              <Button component="label" variant="outlined">
                {field.state.value?.name ?? t('extensions.chooseArchive')}
                <input
                  accept=".tar.zst,application/zstd"
                  hidden
                  onChange={(event) => {
                    const archive = event.target.files?.[0] ?? null;
                    field.handleChange(archive);
                    setArchive(archive);
                  }}
                  type="file"
                />
              </Button>
            )}
          </form.Field>
          <Typography color="text.secondary" sx={{ mt: 1 }} variant="body2">
            {t('extensions.archiveHelp')}
          </Typography>
          <Button
            disabled={!canManage || !archive || sideload.isPending}
            sx={{ mt: 2 }}
            type="submit"
            variant="contained"
          >
            {sideload.isPending
              ? t('extensions.installing')
              : t('extensions.installArchive')}
          </Button>
        </form>
      </Paper>
    </PageContainer>
  );
};

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
    onSuccess: () => invalidate(client),
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
        actions={<Link to="/manage/extensions">Back to extensions</Link>}
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
    onSuccess: () => invalidate(client, extensionId),
  });
  const config = useMutation({
    mutationFn: (value: unknown) => configureExtension(extensionId, value),
    onSuccess: () => invalidate(client, extensionId),
  });
  const revoke = useMutation({
    mutationFn: ({ kind, grant }: { kind: string; grant: string }) =>
      revokeExtensionGrant(extensionId, kind, grant),
    onSuccess: () => invalidate(client, extensionId),
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
            ? `v${installation.version} from ${installation.source}`
            : t('extensions.loadingInstallation')
        }
        actions={<Link to="/manage/extensions">Back to extensions</Link>}
      />
      <ErrorNotice error={detail.error} />
      <ErrorNotice error={action.error} />
      <ErrorNotice error={config.error} />
      <ErrorNotice error={revoke.error} />
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
                Enable
              </Button>
              <Button
                disabled={
                  !manage ||
                  action.isPending ||
                  installation!.state !== 'enabled'
                }
                onClick={() => action.mutate({ action: 'disable' })}
              >
                Disable
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
                Quarantine
              </Button>
              <Button
                color="error"
                disabled={!manage || action.isPending}
                onClick={() => action.mutate({ action: 'remove' })}
              >
                Remove
              </Button>
            </Stack>
          </Paper>
          <Paper sx={{ p: 2 }}>
            <Typography variant="h6">Upgrade</Typography>
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
                    Upgrade
                  </Button>
                </Stack>
              ))}
          </Paper>
          <Paper sx={{ p: 2 }}>
            <Typography variant="h6">Configuration</Typography>
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
                Save configuration
              </Button>
            </form>
          </Paper>
          <Paper sx={{ p: 2 }}>
            <Typography variant="h6">Permissions</Typography>
            {detail.data.grants.map((grant) => (
              <Stack
                direction="row"
                key={`${grant.grant_kind}:${grant.grant_id}`}
                spacing={1}
                sx={{ alignItems: 'center' }}
              >
                <Chip label={`${grant.grant_kind}: ${grant.grant_id}`} />
                <Button
                  disabled={!manage || revoke.isPending}
                  onClick={() =>
                    revoke.mutate({
                      kind: grant.grant_kind,
                      grant: grant.grant_id,
                    })
                  }
                >
                  Revoke
                </Button>
              </Stack>
            ))}
            <DeclaredPermissions
              extensionId={extensionId}
              manifest={installation!.manifest}
              grants={detail.data.grants}
              enabled={manage}
              onGrant={() => invalidate(client, extensionId)}
            />
          </Paper>
          <Paper sx={{ p: 2 }}>
            <Typography variant="h6">Lifecycle and health</Typography>
            {detail.data.lifecycle.length === 0 ? (
              <Typography color="text.secondary">
                No lifecycle history.
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
                    {item.actor_user_id ? `· actor ${item.actor_user_id}` : ''}{' '}
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
const DeclaredPermissions = ({
  extensionId,
  manifest,
  grants,
  enabled,
  onGrant,
}: {
  extensionId: string;
  manifest: unknown;
  grants: {
    grant_kind:
      'capability' | 'host_permission' | 'event_publish' | 'event_subscribe';
    grant_id: string;
  }[];
  enabled: boolean;
  onGrant: () => void;
}) => {
  const { t } = useTranslation();
  const grant = useMutation({
    mutationFn: ({
      kind,
      id,
    }: {
      kind:
        'capability' | 'host_permission' | 'event_publish' | 'event_subscribe';
      id: string;
    }) => grantExtension(extensionId, kind, id),
    onSuccess: onGrant,
  });
  const value = manifest as {
    permissions?: string[];
    optional_permissions?: string[];
    host_permissions?: { id: string }[];
    optional_host_permissions?: { id: string }[];
    event_contracts?: {
      exports?: { id: string }[];
      consumes?: { provider: string; contract: string }[];
    };
  };
  const requested: Array<
    readonly [
      'capability' | 'host_permission' | 'event_publish' | 'event_subscribe',
      string,
    ]
  > = [...(value.permissions ?? []), ...(value.optional_permissions ?? [])].map(
    (id) => ['capability', id] as const,
  );
  requested.push(
    ...[
      ...(value.host_permissions ?? []),
      ...(value.optional_host_permissions ?? []),
    ].map((item) => ['host_permission', item.id] as const),
    ...(value.event_contracts?.exports ?? []).map(
      (item) => ['event_publish', item.id] as const,
    ),
    ...(value.event_contracts?.consumes ?? []).map(
      (item) =>
        ['event_subscribe', `${item.provider}:${item.contract}`] as const,
    ),
  );
  return (
    <Stack spacing={1} sx={{ mt: 1 }}>
      <ErrorNotice error={grant.error} />
      {requested
        .filter(
          ([kind, id]) =>
            !grants.some(
              (grant) => grant.grant_kind === kind && grant.grant_id === id,
            ),
        )
        .map(([kind, id]) => (
          <Stack direction="row" key={`${kind}:${id}`} spacing={1}>
            <Chip label={t('extensions.requestedGrant', { kind, id })} />
            <Button
              disabled={!enabled || grant.isPending}
              onClick={() => grant.mutate({ kind, id })}
            >
              Grant
            </Button>
          </Stack>
        ))}
    </Stack>
  );
};
