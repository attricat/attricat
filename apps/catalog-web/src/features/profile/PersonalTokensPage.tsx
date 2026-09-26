import { useForm } from '@tanstack/react-form';
import { useNavigate } from '@tanstack/react-router';
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
  Paper,
  Stack,
  TextField,
  Typography,
} from '@mui/material';
import { useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { copyToClipboard } from '../../components/clipboard';
import { SettingsPage } from '../../components/CenteredPage';
import { useToast } from '../../components/useToast';
import { currentSession } from '../auth/api';
import { authQueryKeys } from '../auth/queryKeys';
import { createToken, listTokenPermissions } from './api';
import { profileQueryKeys } from './queryKeys';

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
      'entities.read',
      'entities.write',
      'entities.publish',
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

export const PersonalTokensPage = () => {
  const { t } = useTranslation();
  const { show } = useToast();
  const navigate = useNavigate();
  const client = useQueryClient();
  const session = useQuery({
    queryKey: authQueryKeys.session(),
    queryFn: currentSession,
  });
  const canManage = session.data?.capabilities?.tokens_manage === true;
  const permissions = useQuery({
    enabled: canManage,
    queryKey: profileQueryKeys.tokenPermissions(),
    queryFn: listTokenPermissions,
  });
  const [secret, setSecret] = useState<string>();
  const creating = useRef(false);
  const [isCreating, setIsCreating] = useState(false);
  const [error, setError] = useState<string>();
  const form = useForm({
    defaultValues: { label: '', permissions: [] as string[], expires_at: '' },
    onSubmit: async ({ value }) => {
      if (creating.current) return;
      creating.current = true;
      setIsCreating(true);
      setError(undefined);
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
        show({ message: t('profile.tokenCreated'), severity: 'success' });
        void client.invalidateQueries({ queryKey: profileQueryKeys.tokens() });
      } catch (reason) {
        setError(
          reason instanceof Error ? reason.message : t('profile.createFailed'),
        );
      } finally {
        creating.current = false;
        setIsCreating(false);
      }
    },
  });

  if (!canManage) {
    return (
      <SettingsPage>
        <Typography variant="h4">{t('profile.createToken')}</Typography>
        <Alert severity="info" sx={{ mt: 3 }}>
          {t('profile.tokenUnavailable')}
        </Alert>
      </SettingsPage>
    );
  }

  return (
    <SettingsPage>
      <Typography variant="h4">{t('profile.createToken')}</Typography>
      <SecretDialog
        onClose={() =>
          void navigate({ to: '/profile', hash: 'personal-api-tokens' })
        }
        secret={secret}
      />
      <Paper
        component="form"
        onSubmit={(event) => {
          event.preventDefault();
          form.handleSubmit();
        }}
        sx={{ mt: 3, p: 2 }}
      >
        <Stack spacing={1}>
          {error && <Alert severity="error">{error}</Alert>}
          {permissions.isError && (
            <Alert severity="error">{permissions.error.message}</Alert>
          )}
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
          <Button disabled={isCreating} type="submit" variant="contained">
            {t('profile.create')}
          </Button>
        </Stack>
      </Paper>
    </SettingsPage>
  );
};
