import {
  Alert,
  Box,
  Button,
  Chip,
  Link,
  Paper,
  Stack,
  TextField,
  Typography,
} from '@mui/material';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import {
  KeyRoundIcon,
  PlayIcon,
  PlusIcon,
  RefreshCwIcon,
  Trash2Icon,
} from 'lucide-react';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { EmptyState } from '../../components/EmptyState';
import { ExtensionConnectionIcon } from '../../components/systemIcons';
import { Timestamp } from '../../time/Timestamp';
import {
  connectionAction,
  deleteConnection,
  exchangeAuthorizationCode,
  listConnections,
  updateConnection,
} from './api';
import { ConnectionDialog } from './ConnectionDialog';
import { extensionExportQueryKeys } from './queryKeys';
import type { ExtensionConnection, ManifestExports } from './schemas';

const statusColors = {
  active: 'success',
  pending_authorization: 'warning',
  failed: 'error',
  revoked: 'default',
} as const satisfies Record<ExtensionConnection['status'], string>;

const ConnectionRow = ({
  canManage,
  connection,
  extensionId,
}: {
  canManage: boolean;
  connection: ExtensionConnection;
  extensionId: string;
}) => {
  const { t } = useTranslation();
  const client = useQueryClient();
  const [code, setCode] = useState('');
  const [redirectUri, setRedirectUri] = useState('');
  const [secret, setSecret] = useState('');
  const refresh = () =>
    client.invalidateQueries({
      queryKey: extensionExportQueryKeys.connections(extensionId),
    });
  const action = useMutation({
    mutationFn: (
      name: 'device-authorization' | 'device-authorization/poll' | 'test',
    ) => connectionAction(extensionId, connection.connection, name),
    onSuccess: refresh,
  });
  const exchange = useMutation({
    mutationFn: () =>
      exchangeAuthorizationCode(extensionId, connection.connection, {
        code,
        redirect_uri: redirectUri,
      }),
    onSuccess: () => {
      setCode('');
      return refresh();
    },
  });
  const rotate = useMutation({
    mutationFn: () =>
      updateConnection(
        extensionId,
        connection.connection,
        connection.grant_type === 'refresh_token'
          ? { refresh_token: secret }
          : { client_secret: secret },
      ),
    onSuccess: () => {
      setSecret('');
      return refresh();
    },
  });
  const remove = useMutation({
    mutationFn: () => deleteConnection(extensionId, connection.connection),
    onSuccess: refresh,
  });
  const error = action.error ?? exchange.error ?? rotate.error ?? remove.error;
  const busy =
    action.isPending ||
    exchange.isPending ||
    rotate.isPending ||
    remove.isPending;
  return (
    <Box sx={{ borderTop: 1, borderColor: 'divider', py: 2 }}>
      <Stack
        direction={{ xs: 'column', sm: 'row' }}
        spacing={1}
        sx={{ alignItems: { sm: 'center' } }}
      >
        <Box sx={{ flex: 1, minWidth: 0 }}>
          <Typography sx={{ overflowWrap: 'anywhere' }}>
            {connection.display_name}
          </Typography>
          <Typography color="text.secondary" variant="body2">
            {connection.connection} · {connection.kind} ·{' '}
            {t(`extensionExports.grantTypes.${connection.grant_type}`)}
          </Typography>
        </Box>
        <Chip
          color={statusColors[connection.status]}
          label={t(`extensionExports.connectionStatus.${connection.status}`)}
          size="small"
          variant="outlined"
        />
      </Stack>
      <Typography color="text.secondary" sx={{ mt: 1 }} variant="body2">
        {connection.last_refreshed_at ? (
          <>
            {t('extensionExports.lastRefreshed')}{' '}
            <Timestamp value={connection.last_refreshed_at} />
          </>
        ) : (
          t('extensionExports.neverRefreshed')
        )}
        {connection.has_client_secret &&
          ` · ${t('extensionExports.hasSecret')}`}
        {connection.has_refresh_token &&
          ` · ${t('extensionExports.hasRefreshToken')}`}
      </Typography>
      {connection.last_error && (
        <Alert severity="warning" sx={{ mt: 1 }}>
          {connection.last_error}
        </Alert>
      )}
      {error && (
        <Alert severity="error" sx={{ mt: 1 }}>
          {error.message}
        </Alert>
      )}
      {connection.device_user_code && connection.device_verification_uri && (
        <Alert severity="info" sx={{ mt: 1 }}>
          {t('extensionExports.deviceInstructions', {
            code: connection.device_user_code,
          })}{' '}
          <Link
            href={connection.device_verification_uri}
            rel="noopener noreferrer"
            target="_blank"
          >
            {connection.device_verification_uri}
          </Link>
        </Alert>
      )}
      {canManage && (
        <Stack
          direction="row"
          spacing={1}
          sx={{ flexWrap: 'wrap', mt: 1 }}
          useFlexGap
        >
          {connection.grant_type === 'device_code' && (
            <>
              <Button
                disabled={busy}
                size="small"
                startIcon={<KeyRoundIcon />}
                onClick={() => action.mutate('device-authorization')}
              >
                {t('extensionExports.startAuthorization')}
              </Button>
              {connection.device_user_code && (
                <Button
                  disabled={busy}
                  size="small"
                  startIcon={<RefreshCwIcon />}
                  onClick={() => action.mutate('device-authorization/poll')}
                >
                  {t('extensionExports.checkAuthorization')}
                </Button>
              )}
            </>
          )}
          {connection.status === 'active' && (
            <Button
              disabled={busy}
              size="small"
              startIcon={<PlayIcon />}
              onClick={() => action.mutate('test')}
            >
              {t('extensionExports.testConnection')}
            </Button>
          )}
          <Button
            color="error"
            disabled={busy}
            size="small"
            startIcon={<Trash2Icon />}
            onClick={() => remove.mutate()}
          >
            {t('extensionExports.deleteConnection')}
          </Button>
        </Stack>
      )}
      {canManage && connection.grant_type === 'authorization_code' && (
        <Stack
          component="form"
          direction={{ xs: 'column', sm: 'row' }}
          spacing={1}
          sx={{ mt: 1 }}
          onSubmit={(event) => {
            event.preventDefault();
            exchange.mutate();
          }}
        >
          <TextField
            label={t('extensionExports.authorizationCode')}
            required
            size="small"
            value={code}
            onChange={(event) => setCode(event.target.value)}
          />
          <TextField
            label={t('extensionExports.redirectUri')}
            required
            size="small"
            type="url"
            value={redirectUri}
            onChange={(event) => setRedirectUri(event.target.value)}
          />
          <Button disabled={busy} type="submit">
            {t('extensionExports.exchangeCode')}
          </Button>
        </Stack>
      )}
      {canManage && connection.grant_type !== 'device_code' && (
        <Stack
          component="form"
          direction={{ xs: 'column', sm: 'row' }}
          spacing={1}
          sx={{ mt: 1 }}
          onSubmit={(event) => {
            event.preventDefault();
            rotate.mutate();
          }}
        >
          <TextField
            autoComplete="new-password"
            label={
              connection.grant_type === 'refresh_token'
                ? t('extensionExports.replaceRefreshToken')
                : t('extensionExports.replaceSecret')
            }
            size="small"
            type="password"
            value={secret}
            onChange={(event) => setSecret(event.target.value)}
          />
          <Button disabled={busy || !secret} type="submit">
            {t('extensionExports.replace')}
          </Button>
        </Stack>
      )}
    </Box>
  );
};

/**
 * Host-managed destination accounts. Operators create them and complete
 * their authorization; credentials are write-only and never reach the
 * extension.
 */
export const ExtensionConnectionsSection = ({
  canManage,
  extensionId,
  manifest,
}: {
  canManage: boolean;
  extensionId: string;
  manifest: ManifestExports;
}) => {
  const { t } = useTranslation();
  const client = useQueryClient();
  const kinds = manifest.server?.connections ?? [];
  const [creating, setCreating] = useState(false);
  const connections = useQuery({
    queryKey: extensionExportQueryKeys.connections(extensionId),
    queryFn: () => listConnections(extensionId),
    enabled: kinds.length > 0,
  });
  if (!kinds.length) return null;
  return (
    <Paper sx={{ p: 2 }}>
      <Stack direction="row" sx={{ alignItems: 'center', mb: 1 }}>
        <Typography sx={{ flex: 1 }} variant="h6">
          {t('extensionExports.connections')}
        </Typography>
        {canManage && (
          <Button startIcon={<PlusIcon />} onClick={() => setCreating(true)}>
            {t('extensionExports.addConnection')}
          </Button>
        )}
      </Stack>
      <Typography color="text.secondary" variant="body2">
        {t('extensionExports.connectionsDescription')}
      </Typography>
      {connections.error && (
        <Alert severity="error">{connections.error.message}</Alert>
      )}
      {connections.data?.length === 0 && (
        <EmptyState
          description={t('extensionExports.noConnectionsDescription')}
          icon={ExtensionConnectionIcon}
          title={t('extensionExports.noConnections')}
        />
      )}
      {connections.data?.map((connection) => (
        <ConnectionRow
          canManage={canManage}
          connection={connection}
          extensionId={extensionId}
          key={connection.id}
        />
      ))}
      {creating && (
        <ConnectionDialog
          extensionId={extensionId}
          kinds={kinds}
          onClose={() => setCreating(false)}
          onCreated={() => {
            setCreating(false);
            void client.invalidateQueries({
              queryKey: extensionExportQueryKeys.connections(extensionId),
            });
          }}
        />
      )}
    </Paper>
  );
};
