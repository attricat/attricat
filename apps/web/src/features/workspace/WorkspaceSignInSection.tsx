import {
  Alert,
  Box,
  Button,
  CircularProgress,
  FormControl,
  FormControlLabel,
  FormHelperText,
  FormLabel,
  Radio,
  RadioGroup,
  Stack,
  Switch,
  TextField,
  Typography,
} from '@mui/material';
import { useForm } from '@tanstack/react-form';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { ApiErrorAlert } from '../../components/ApiErrorAlert';
import { ConfirmDialog } from '../../components/ConfirmDialog';
import {
  leaveForProvider,
  rememberReturnLocation,
} from '../auth/returnLocation';
import { startIdentityLink } from '../auth/api';
import {
  deleteIdentityProvider,
  getIdentityProvider,
  putIdentityProvider,
  updateIdentityProviderState,
} from './api';
import { passwordSignInPolicies } from './constants';
import { workspaceQueryKeys } from './queryKeys';
import type { IdentityProvider } from './schemas';

const ProviderForm = ({ provider }: { provider: IdentityProvider | null }) => {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const [notice, setNotice] = useState('');
  const save = useMutation({
    meta: { toast: false },
    mutationFn: putIdentityProvider,
    onSuccess: (saved) => {
      queryClient.setQueryData(workspaceQueryKeys.identityProvider(), saved);
      form.setFieldValue('client_secret', '');
      setNotice(t('workspace.signInSaved'));
    },
  });
  const form = useForm({
    defaultValues: {
      client_id: provider?.client_id ?? '',
      client_secret: '',
      display_name: provider?.display_name ?? '',
      issuer: provider?.issuer ?? '',
    },
    onSubmit: ({ value }) => {
      setNotice('');
      save.mutate({
        client_id: value.client_id,
        display_name: value.display_name,
        issuer: value.issuer,
        // Leaving it blank keeps the stored secret.
        ...(value.client_secret ? { client_secret: value.client_secret } : {}),
      });
    },
  });
  return (
    <Stack
      component="form"
      onSubmit={(event) => {
        event.preventDefault();
        void form.handleSubmit();
      }}
      spacing={2}
    >
      <form.Field name="display_name">
        {(field) => (
          <TextField
            helperText={t('workspace.signInDisplayNameHelp')}
            label={t('workspace.signInDisplayName')}
            onChange={(event) => field.handleChange(event.target.value)}
            required
            value={field.state.value}
          />
        )}
      </form.Field>
      <form.Field name="issuer">
        {(field) => (
          <TextField
            helperText={t('workspace.signInIssuerHelp')}
            label={t('workspace.signInIssuer')}
            onChange={(event) => field.handleChange(event.target.value)}
            required
            type="url"
            value={field.state.value}
          />
        )}
      </form.Field>
      <form.Field name="client_id">
        {(field) => (
          <TextField
            autoComplete="off"
            label={t('workspace.signInClientId')}
            onChange={(event) => field.handleChange(event.target.value)}
            required
            value={field.state.value}
          />
        )}
      </form.Field>
      <form.Field name="client_secret">
        {(field) => (
          <TextField
            autoComplete="new-password"
            helperText={
              provider ? t('workspace.signInClientSecretKeep') : undefined
            }
            label={t('workspace.signInClientSecret')}
            onChange={(event) => field.handleChange(event.target.value)}
            required={!provider}
            type="password"
            value={field.state.value}
          />
        )}
      </form.Field>
      <ApiErrorAlert error={save.error} />
      <Box>
        <Button disabled={save.isPending} type="submit" variant="contained">
          {t('workspace.signInSave')}
        </Button>
      </Box>
      <Typography role="status" variant="body2">
        {notice}
      </Typography>
    </Stack>
  );
};

const ProviderState = ({ provider }: { provider: IdentityProvider }) => {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const [confirmingDelete, setConfirmingDelete] = useState(false);
  const update = useMutation({
    meta: { toast: false },
    mutationFn: updateIdentityProviderState,
    onSuccess: (saved) =>
      queryClient.setQueryData(workspaceQueryKeys.identityProvider(), saved),
  });
  const testSignIn = useMutation({
    meta: { toast: false },
    mutationFn: startIdentityLink,
    onSuccess: ({ authorization_url }) => {
      rememberReturnLocation();
      leaveForProvider(authorization_url);
    },
  });
  const remove = useMutation({
    meta: { toast: false },
    mutationFn: deleteIdentityProvider,
    onSuccess: () =>
      queryClient.setQueryData(workspaceQueryKeys.identityProvider(), null),
  });
  const verified = provider.verified_at !== null;
  return (
    <Stack spacing={2}>
      {provider.redirect_uri ? (
        <Alert severity="info">
          {t('workspace.signInRedirectUri')}
          <Box
            component="code"
            sx={{ display: 'block', mt: 1, overflowWrap: 'anywhere' }}
          >
            {provider.redirect_uri}
          </Box>
        </Alert>
      ) : (
        <Alert severity="warning">{t('workspace.signInUnavailable')}</Alert>
      )}
      <Typography component="h3" variant="subtitle1">
        {t('workspace.signInTestTitle')}
      </Typography>
      <Typography color="text.secondary" variant="body2">
        {verified
          ? t('workspace.signInVerified')
          : t('workspace.signInTestDescription')}
      </Typography>
      <ApiErrorAlert error={testSignIn.error} />
      <Box>
        <Button
          disabled={
            !provider.redirect_uri ||
            testSignIn.isPending ||
            testSignIn.isSuccess
          }
          onClick={() => testSignIn.mutate()}
          variant="outlined"
        >
          {t('workspace.signInTest')}
        </Button>
      </Box>
      <FormControl disabled={!verified || update.isPending}>
        <FormControlLabel
          control={
            <Switch
              checked={provider.enabled}
              onChange={(event) =>
                update.mutate({ enabled: event.target.checked })
              }
            />
          }
          label={t('workspace.signInEnabled')}
        />
        {!verified && (
          <FormHelperText>
            {t('workspace.signInEnableNeedsTest')}
          </FormHelperText>
        )}
      </FormControl>
      <FormControl disabled={!provider.enabled || update.isPending}>
        <FormLabel>{t('workspace.signInPasswordPolicy')}</FormLabel>
        <RadioGroup
          onChange={(event) =>
            update.mutate({ password_sign_in: event.target.value })
          }
          value={provider.password_sign_in}
        >
          <FormControlLabel
            control={<Radio />}
            label={t('workspace.signInPasswordAll')}
            value={passwordSignInPolicies.all}
          />
          <FormControlLabel
            control={<Radio />}
            label={t('workspace.signInPasswordOwners')}
            value={passwordSignInPolicies.owners}
          />
        </RadioGroup>
      </FormControl>
      <ApiErrorAlert error={update.error} />
      <Box>
        <Button color="error" onClick={() => setConfirmingDelete(true)}>
          {t('workspace.signInDelete')}
        </Button>
      </Box>
      {confirmingDelete && (
        <ConfirmDialog
          confirmLabel={t('workspace.signInDelete')}
          destructive
          error={remove.error}
          isPending={remove.isPending}
          onClose={() => setConfirmingDelete(false)}
          onConfirm={() => remove.mutate()}
          title={t('workspace.signInDeleteTitle')}
        >
          {t('workspace.signInDeleteDescription', {
            provider: provider.display_name,
          })}
        </ConfirmDialog>
      )}
    </Stack>
  );
};

export const WorkspaceSignInSection = ({
  canManage,
}: {
  canManage: boolean;
}) => {
  const { t } = useTranslation();
  const provider = useQuery({
    queryKey: workspaceQueryKeys.identityProvider(),
    queryFn: getIdentityProvider,
    enabled: canManage,
  });
  if (!canManage)
    return <Alert severity="error">{t('workspace.notAuthorizedSignIn')}</Alert>;
  return (
    <Stack spacing={4} sx={{ mt: 3 }}>
      <Stack spacing={1}>
        <Typography component="h2" variant="h6">
          {t('workspace.signInTitle')}
        </Typography>
        <Typography color="text.secondary">
          {t('workspace.signInDescription')}
        </Typography>
      </Stack>
      {provider.isPending && (
        <CircularProgress aria-label={t('workspace.signInLoading')} />
      )}
      <ApiErrorAlert error={provider.error} />
      {provider.isSuccess && (
        <ProviderForm
          key={provider.data?.updated_at ?? 'new'}
          provider={provider.data}
        />
      )}
      {provider.data && <ProviderState provider={provider.data} />}
    </Stack>
  );
};
