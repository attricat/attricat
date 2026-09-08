import { useForm } from '@tanstack/react-form';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import {
  Alert,
  Box,
  Button,
  Checkbox,
  Dialog,
  DialogContent,
  DialogTitle,
  FormControlLabel,
  List,
  ListItem,
  ListItemText,
  Paper,
  Stack,
  TextField,
  Typography,
} from '@mui/material';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { copyToClipboard } from '../../components/clipboard';
import { useToast } from '../../components/useToast';
import i18n from '../../i18n';
import { currentSession } from '../auth/api';
import { authQueryKeys } from '../auth/query-keys';
import {
  createToken,
  listTokenPermissions,
  listTokens,
  revokeToken,
} from './api';
import { profileQueryKeys } from './query-keys';

const formatTime = (value: string | null) =>
  value
    ? new Intl.DateTimeFormat(i18n.language, {
        dateStyle: 'medium',
        timeStyle: 'medium',
      }).format(new Date(value))
    : i18n.t('profile.never');

const tokenPermissionPresets = [
  {
    nameKey: 'profile.catalogGenerator',
    descriptionKey: 'profile.catalogGeneratorDescription',
    permissions: [
      'blueprints.read',
      'blueprints.write',
      'blueprints.publish',
      'contexts.read',
      'contexts.write',
      'entities.write',
    ],
  },
  {
    nameKey: 'profile.readOnlyCatalog',
    descriptionKey: 'profile.readOnlyCatalogDescription',
    permissions: ['blueprints.read', 'contexts.read', 'entities.read'],
  },
  {
    nameKey: 'profile.entityImporter',
    descriptionKey: 'profile.entityImporterDescription',
    permissions: ['blueprints.read', 'contexts.read', 'entities.write'],
  },
];

const SecretDialog = ({
  secret,
  onClose,
}: {
  secret?: string;
  onClose: () => void;
}) => {
  const { t } = useTranslation();
  const { show } = useToast();
  const copySecret = async () => {
    try {
      await copyToClipboard(secret ?? '');
      show({ message: t('common.copied'), severity: 'success' });
    } catch {
      show({ message: t('common.copyFailed'), severity: 'error' });
    }
  };
  return (
    <Dialog onClose={onClose} open={Boolean(secret)}>
      <DialogTitle>{t('profile.copySecretTitle')}</DialogTitle>
      <DialogContent>
        <Stack spacing={2} sx={{ minWidth: 360 }}>
          <Alert severity="warning">{t('profile.copySecretWarning')}</Alert>
          <TextField
            slotProps={{ input: { readOnly: true } }}
            value={secret ?? ''}
          />
          <Button onClick={() => void copySecret()} variant="contained">
            {t('profile.copySecret')}
          </Button>
        </Stack>
      </DialogContent>
    </Dialog>
  );
};

const PersonalTokens = ({ canManage }: { canManage: boolean }) => {
  const { t } = useTranslation();
  const client = useQueryClient();
  const [secret, setSecret] = useState<string>();
  const [error, setError] = useState<string>();
  const tokens = useQuery({
    enabled: canManage,
    queryKey: profileQueryKeys.tokens(),
    queryFn: listTokens,
  });
  const permissions = useQuery({
    enabled: canManage,
    queryKey: profileQueryKeys.tokenPermissions(),
    queryFn: listTokenPermissions,
  });
  const refresh = () =>
    client.invalidateQueries({ queryKey: profileQueryKeys.tokens() });
  const form = useForm({
    defaultValues: { label: '', permissions: [] as string[], expires_at: '' },
    onSubmit: async ({ value }) => {
      try {
        const expiresAt = value.expires_at
          ? new Date(value.expires_at)
          : undefined;
        if (
          expiresAt &&
          (Number.isNaN(expiresAt.getTime()) || expiresAt <= new Date())
        ) {
          throw new Error(t('profile.tokenExpiry'));
        }
        const token = await createToken({
          label: value.label,
          permissions: value.permissions,
          ...(expiresAt ? { expires_at: expiresAt.toISOString() } : {}),
        });
        setSecret(token.secret);
        refresh();
      } catch (reason) {
        setError(
          reason instanceof Error ? reason.message : t('profile.createFailed'),
        );
      }
    },
  });

  if (!canManage) {
    return <Alert severity="info">{t('profile.tokenUnavailable')}</Alert>;
  }
  return (
    <Stack spacing={2}>
      <SecretDialog onClose={() => setSecret(undefined)} secret={secret} />
      {error && <Alert severity="error">{error}</Alert>}
      {tokens.isError && <Alert severity="error">{tokens.error.message}</Alert>}
      {permissions.isError && (
        <Alert severity="error">{permissions.error.message}</Alert>
      )}
      <Paper>
        <List aria-label={t('profile.tokenList')}>
          {tokens.data?.map((token) => (
            <ListItem
              divider
              key={token.id}
              secondaryAction={
                !token.revoked_at && (
                  <Button
                    color="error"
                    onClick={() =>
                      revokeToken(token.id)
                        .then(refresh)
                        .catch((reason) => setError(reason.message))
                    }
                  >
                    {t('profile.revoke')}
                  </Button>
                )
              }
            >
              <ListItemText
                primary={token.label}
                secondary={t('profile.tokenDetails', {
                  permissions: token.permissions.join(', '),
                  created: formatTime(token.created_at),
                  lastUsed: formatTime(token.last_used_at),
                  expires: formatTime(token.expires_at),
                  revoked: token.revoked_at
                    ? formatTime(token.revoked_at)
                    : t('profile.no'),
                })}
              />
            </ListItem>
          ))}
          {tokens.data?.length === 0 && (
            <ListItem>
              <ListItemText primary={t('profile.noTokens')} />
            </ListItem>
          )}
        </List>
      </Paper>
      <Paper
        component="form"
        onSubmit={(event) => {
          event.preventDefault();
          form.handleSubmit();
        }}
        sx={{ p: 2 }}
      >
        <Stack spacing={1}>
          <Typography variant="h6">{t('profile.createToken')}</Typography>
          <form.Field name="label">
            {(field) => (
              <TextField
                label={t('profile.label')}
                onChange={(event) => field.handleChange(event.target.value)}
                value={field.state.value}
              />
            )}
          </form.Field>
          <form.Field name="permissions">
            {(field) => {
              const availablePermissions = new Set(
                permissions.data?.map((permission) => permission.code),
              );
              return (
                <>
                  <Typography variant="subtitle2">
                    {t('profile.permissionPresets')}
                  </Typography>
                  <Stack direction={{ sm: 'row', xs: 'column' }} spacing={1}>
                    {tokenPermissionPresets.map((preset) => {
                      const unavailable = permissions.data
                        ? preset.permissions.some(
                            (code) => !availablePermissions.has(code),
                          )
                        : false;
                      return (
                        <Box key={preset.nameKey} sx={{ flex: 1 }}>
                          <Button
                            disabled={!permissions.data || unavailable}
                            fullWidth
                            onClick={() =>
                              field.handleChange(preset.permissions)
                            }
                            type="button"
                            variant="outlined"
                          >
                            {t(preset.nameKey)}
                          </Button>
                          <Typography variant="caption">
                            {unavailable
                              ? t('profile.unavailable')
                              : t(preset.descriptionKey)}
                          </Typography>
                        </Box>
                      );
                    })}
                  </Stack>
                  <Typography variant="subtitle2">
                    {t('profile.customPermissions')}
                  </Typography>
                  {permissions.data?.map((permission) => (
                    <FormControlLabel
                      control={
                        <Checkbox
                          checked={field.state.value.includes(permission.code)}
                          onChange={(event) =>
                            field.handleChange(
                              event.target.checked
                                ? [...field.state.value, permission.code]
                                : field.state.value.filter(
                                    (code) => code !== permission.code,
                                  ),
                            )
                          }
                        />
                      }
                      key={permission.code}
                      label={permission.code}
                    />
                  ))}
                </>
              );
            }}
          </form.Field>
          <form.Field name="expires_at">
            {(field) => (
              <TextField
                slotProps={{ inputLabel: { shrink: true } }}
                label={t('profile.expiresAt')}
                onChange={(event) => field.handleChange(event.target.value)}
                type="datetime-local"
                value={field.state.value}
              />
            )}
          </form.Field>
          <Button type="submit" variant="contained">
            {t('profile.create')}
          </Button>
        </Stack>
      </Paper>
    </Stack>
  );
};

export const ProfilePage = () => {
  const { t } = useTranslation();
  const session = useQuery({
    queryKey: authQueryKeys.session(),
    queryFn: currentSession,
  });
  const account = session.data;
  return (
    <Box sx={{ maxWidth: 1000, mx: 'auto', p: 3 }}>
      <Typography variant="h4">{t('profile.title')}</Typography>
      {session.isError && (
        <Alert severity="error">{session.error.message}</Alert>
      )}
      <Stack spacing={3} sx={{ mt: 3 }}>
        <Paper sx={{ p: 2 }}>
          <Typography variant="h6">{t('profile.accountDetails')}</Typography>
          <Typography>
            {t('profile.displayName', {
              value: account?.display_name ?? t('profile.notSet'),
            })}
          </Typography>
          <Typography>
            {t('profile.email', {
              value: account?.email ?? t('profile.loading'),
            })}
          </Typography>
          <Typography>
            {t('profile.userId', {
              value: account?.user_id ?? t('profile.loading'),
            })}
          </Typography>
          <Typography>
            {t('profile.activeWorkspace', {
              value: account?.workspace_id ?? t('profile.loading'),
            })}
          </Typography>
        </Paper>
        <Box id="personal-api-tokens">
          <Typography gutterBottom variant="h5">
            {t('profile.tokenList')}
          </Typography>
          <PersonalTokens
            canManage={account?.capabilities?.tokens_manage === true}
          />
        </Box>
      </Stack>
    </Box>
  );
};
