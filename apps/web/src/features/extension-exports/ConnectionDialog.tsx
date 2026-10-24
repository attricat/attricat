import {
  Alert,
  Button,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  MenuItem,
  Stack,
  TextField,
} from '@mui/material';
import { useMutation } from '@tanstack/react-query';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { createConnection, type NewConnection } from './api';
import type { ConnectionGrantType } from './constants';
import type { ManifestExports } from './schemas';

type ConnectionKind = NonNullable<
  ManifestExports['server']
>['connections'][number];

const parseParameters = (value: string) => {
  if (!value.trim()) return {};
  const parsed: unknown = JSON.parse(value);
  if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed)) {
    throw new Error('object expected');
  }
  return parsed as Record<string, unknown>;
};

/**
 * Creates a destination account of one of the extension's connection kinds.
 * Secrets go to the host once and are never shown again.
 */
export const ConnectionDialog = ({
  extensionId,
  kinds,
  onClose,
  onCreated,
}: {
  extensionId: string;
  kinds: ConnectionKind[];
  onClose: () => void;
  onCreated: () => void;
}) => {
  const { t } = useTranslation();
  const [kind, setKind] = useState(kinds[0]?.id ?? '');
  const grantTypes = kinds.find((item) => item.id === kind)?.grant_types ?? [];
  const [grantType, setGrantType] = useState<ConnectionGrantType>(
    grantTypes[0] ?? 'client_credentials',
  );
  const [fields, setFields] = useState({
    connection: '',
    display_name: '',
    token_url: '',
    device_authorization_url: '',
    client_id: '',
    client_secret: '',
    refresh_token: '',
    scope: '',
    parameters: '',
  });
  const [parametersError, setParametersError] = useState(false);
  const create = useMutation({
    mutationFn: (input: NewConnection) => createConnection(extensionId, input),
    onSuccess: onCreated,
  });
  const set =
    (name: keyof typeof fields) =>
    (event: React.ChangeEvent<HTMLInputElement>) =>
      setFields((current) => ({ ...current, [name]: event.target.value }));
  const submit = () => {
    let parameters: Record<string, unknown>;
    try {
      parameters = parseParameters(fields.parameters);
    } catch {
      setParametersError(true);
      return;
    }
    setParametersError(false);
    const optional = (value: string) => (value.trim() ? value : undefined);
    create.mutate({
      connection: fields.connection.trim(),
      kind,
      display_name: fields.display_name.trim() || fields.connection.trim(),
      grant_type: grantType,
      token_url: fields.token_url.trim(),
      device_authorization_url:
        grantType === 'device_code'
          ? optional(fields.device_authorization_url)
          : undefined,
      client_id: fields.client_id.trim(),
      client_secret: optional(fields.client_secret),
      refresh_token:
        grantType === 'refresh_token'
          ? optional(fields.refresh_token)
          : undefined,
      scope: optional(fields.scope),
      parameters,
    });
  };
  return (
    <Dialog fullWidth maxWidth="sm" open onClose={onClose}>
      <DialogTitle>{t('extensionExports.addConnection')}</DialogTitle>
      <DialogContent>
        <Stack
          component="form"
          id="extension-connection-form"
          spacing={2}
          sx={{ mt: 1 }}
          onSubmit={(event) => {
            event.preventDefault();
            submit();
          }}
        >
          {create.error && (
            <Alert severity="error">{create.error.message}</Alert>
          )}
          <TextField
            label={t('extensionExports.connectionKind')}
            select
            value={kind}
            onChange={(event) => {
              setKind(event.target.value);
              const next = kinds.find((item) => item.id === event.target.value);
              setGrantType(next?.grant_types[0] ?? 'client_credentials');
            }}
          >
            {kinds.map((item) => (
              <MenuItem key={item.id} value={item.id}>
                {item.title}
              </MenuItem>
            ))}
          </TextField>
          <TextField
            helperText={t('extensionExports.connectionNameHelp')}
            label={t('extensionExports.connectionName')}
            required
            value={fields.connection}
            onChange={set('connection')}
          />
          <TextField
            label={t('extensionExports.displayName')}
            value={fields.display_name}
            onChange={set('display_name')}
          />
          <TextField
            label={t('extensionExports.grantType')}
            select
            value={grantType}
            onChange={(event) =>
              setGrantType(event.target.value as ConnectionGrantType)
            }
          >
            {grantTypes.map((item) => (
              <MenuItem key={item} value={item}>
                {t(`extensionExports.grantTypes.${item}`)}
              </MenuItem>
            ))}
          </TextField>
          <TextField
            label={t('extensionExports.tokenUrl')}
            required
            type="url"
            value={fields.token_url}
            onChange={set('token_url')}
          />
          {grantType === 'device_code' && (
            <TextField
              label={t('extensionExports.deviceAuthorizationUrl')}
              required
              type="url"
              value={fields.device_authorization_url}
              onChange={set('device_authorization_url')}
            />
          )}
          <TextField
            label={t('extensionExports.clientId')}
            required
            value={fields.client_id}
            onChange={set('client_id')}
          />
          <TextField
            autoComplete="new-password"
            helperText={t('extensionExports.writeOnly')}
            label={t('extensionExports.clientSecret')}
            required={grantType === 'client_credentials'}
            type="password"
            value={fields.client_secret}
            onChange={set('client_secret')}
          />
          {grantType === 'refresh_token' && (
            <TextField
              autoComplete="new-password"
              helperText={t('extensionExports.writeOnly')}
              label={t('extensionExports.refreshToken')}
              type="password"
              value={fields.refresh_token}
              onChange={set('refresh_token')}
            />
          )}
          <TextField
            label={t('extensionExports.scope')}
            value={fields.scope}
            onChange={set('scope')}
          />
          <TextField
            error={parametersError}
            helperText={
              parametersError
                ? t('extensionExports.parametersInvalid')
                : t('extensionExports.parametersHelp')
            }
            label={t('extensionExports.parameters')}
            minRows={2}
            multiline
            value={fields.parameters}
            onChange={set('parameters')}
          />
        </Stack>
      </DialogContent>
      <DialogActions>
        <Button disabled={create.isPending} onClick={onClose}>
          {t('extensionExports.cancel')}
        </Button>
        <Button
          disabled={create.isPending}
          form="extension-connection-form"
          type="submit"
          variant="contained"
        >
          {t('extensionExports.create')}
        </Button>
      </DialogActions>
    </Dialog>
  );
};
