import {
  Alert,
  Button,
  MenuItem,
  Stack,
  TextField,
  Typography,
} from '@mui/material';
import { useForm } from '@tanstack/react-form';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { Link, Navigate, useNavigate } from '@tanstack/react-router';
import { type ReactNode, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { returnToStorageKey } from '../../app/storageKeys';
import { LanguageSwitcher } from '../../components/LanguageSwitcher';
import { AuthFormShell, authFormWidth } from './AuthFormShell';
import {
  type SampleLogins,
  discoverWorkspace,
  fetchSampleLogins,
  login,
} from './api';
import { authQueryKeys } from './queryKeys';

const useLoginSubmission = () => {
  const [submitting, setSubmitting] = useState(false);
  const pending = useRef(false);
  const submit = async (action: () => Promise<void>) => {
    if (pending.current) return;
    pending.current = true;
    setSubmitting(true);
    try {
      await action();
    } finally {
      pending.current = false;
      setSubmitting(false);
    }
  };
  return { submit, submitting };
};

const devLoginDefaults = import.meta.env.DEV
  ? {
      email: 'owner@example.test',
      loginIdentifier: 'default.local',
      password: 'test',
    }
  : { email: '', loginIdentifier: '', password: '' };

/** Settles to `null` without seeded accounts or when the lookup fails. */
const useSampleLogins = () => {
  const query = useQuery({
    queryKey: authQueryKeys.sampleLogins(),
    queryFn: fetchSampleLogins,
    retry: false,
    staleTime: Infinity,
  });
  return { samples: query.data ?? null, pending: query.isPending };
};

/** Demo visitors start as an editor, with owner and admin one pick away;
 * development starts as the owner. */
const defaultSampleRole = (samples: SampleLogins) =>
  samples.demo ? 'editor' : 'owner';

export const WorkspaceLoginPage = () => {
  const { samples, pending } = useSampleLogins();
  if (pending) return null;
  // Demo deployments have one workspace, so sign-in starts at its password step.
  if (samples?.demo)
    return (
      <Navigate
        params={{ identifier: samples.login_identifier }}
        replace
        to="/login/$identifier"
      />
    );
  return (
    <WorkspaceLoginForm
      defaultIdentifier={
        samples?.login_identifier ?? devLoginDefaults.loginIdentifier
      }
    />
  );
};

const WorkspaceLoginForm = ({
  defaultIdentifier,
}: {
  defaultIdentifier: string;
}) => {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const [error, setError] = useState<string>();
  const { submit, submitting } = useLoginSubmission();
  const form = useForm({
    defaultValues: {
      loginIdentifier: defaultIdentifier,
    },
    onSubmit: ({ value }) =>
      submit(async () => {
        try {
          const workspace = await discoverWorkspace(value.loginIdentifier);
          await navigate({
            to: '/login/$identifier',
            params: { identifier: workspace.login_identifier },
          });
        } catch (reason) {
          setError(
            reason instanceof Error
              ? reason.message
              : t('auth.workspaceNotFound'),
          );
        }
      }),
  });
  return (
    <LoginShell onSubmit={() => form.handleSubmit()}>
      <Typography variant="h5">{t('auth.signIn')}</Typography>
      <Typography>{t('auth.workspacePrompt')}</Typography>
      <form.Field name="loginIdentifier">
        {(field) => (
          <TextField
            autoComplete="organization"
            disabled={submitting}
            label={t('auth.workspace')}
            onChange={(event) => field.handleChange(event.target.value)}
            value={field.state.value}
          />
        )}
      </form.Field>
      {error && <Typography color="error">{error}</Typography>}
      <Button disabled={submitting} type="submit" variant="contained">
        {t('auth.continue')}
      </Button>
    </LoginShell>
  );
};

export const PasswordLoginPage = ({ identifier }: { identifier: string }) => {
  const { samples, pending } = useSampleLogins();
  if (pending) return null;
  return (
    <PasswordLoginForm
      identifier={identifier}
      samples={samples?.login_identifier === identifier ? samples : null}
    />
  );
};

const PasswordLoginForm = ({
  identifier,
  samples,
}: {
  identifier: string;
  samples: SampleLogins | null;
}) => {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const [error, setError] = useState<string>();
  const { submit, submitting } = useLoginSubmission();
  const initialSample =
    samples?.accounts.find(({ role }) => role === defaultSampleRole(samples)) ??
    samples?.accounts[0];
  const form = useForm({
    defaultValues: initialSample
      ? { email: initialSample.email, password: samples?.password ?? '' }
      : { email: devLoginDefaults.email, password: devLoginDefaults.password },
    onSubmit: ({ value }) =>
      submit(async () => {
        try {
          const session = await login(identifier, value.email, value.password);
          queryClient.clear();
          queryClient.setQueryData(authQueryKeys.session(), session);
          let returnTo: string | null = null;
          try {
            returnTo = sessionStorage.getItem(returnToStorageKey);
            sessionStorage.removeItem(returnToStorageKey);
          } catch {
            // Signing in also works when browser storage is unavailable.
          }
          await navigate({
            to:
              returnTo?.startsWith('/') && !returnTo.startsWith('//')
                ? returnTo
                : '/',
          });
        } catch (reason) {
          setError(
            reason instanceof Error ? reason.message : t('auth.loginFailed'),
          );
        }
      }),
  });
  return (
    <LoginShell onSubmit={() => form.handleSubmit()}>
      <Typography variant="h5">{t('auth.signInTo', { identifier })}</Typography>
      {samples?.demo ? (
        <Alert severity="info" sx={{ mb: 2 }}>
          {t('auth.demoNotice')}
        </Alert>
      ) : (
        <Button component={Link} to="/login" variant="text">
          {t('auth.changeWorkspace')}
        </Button>
      )}
      {samples && (
        <form.Subscribe selector={(state) => state.values.email}>
          {(email) => (
            // Follows the email field, so typing another address clears it.
            <TextField
              disabled={submitting}
              label={t('auth.sampleAccount')}
              onChange={(event) => {
                form.setFieldValue('email', event.target.value);
                form.setFieldValue('password', samples.password);
              }}
              select
              value={
                samples.accounts.find(
                  (account) => account.email === email.trim().toLowerCase(),
                )?.email ?? ''
              }
            >
              {samples.accounts.map(({ email, role }) => (
                <MenuItem key={email} value={email}>
                  {t(`auth.sampleRoles.${role}`, { defaultValue: role })} ·{' '}
                  {email}
                </MenuItem>
              ))}
            </TextField>
          )}
        </form.Subscribe>
      )}
      <form.Field name="email">
        {(field) => (
          <TextField
            autoComplete="username"
            disabled={submitting}
            label={t('auth.email')}
            onChange={(event) => field.handleChange(event.target.value)}
            type="email"
            value={field.state.value}
          />
        )}
      </form.Field>
      <form.Field name="password">
        {(field) => (
          <TextField
            autoComplete="current-password"
            disabled={submitting}
            label={t('auth.password')}
            onChange={(event) => field.handleChange(event.target.value)}
            type="password"
            value={field.state.value}
          />
        )}
      </form.Field>
      {error && <Typography color="error">{error}</Typography>}
      {!samples?.demo && (
        <Button component={Link} to="/password-reset" variant="text">
          {t('auth.forgotPassword')}
        </Button>
      )}
      <Button disabled={submitting} type="submit" variant="contained">
        {t('auth.signIn')}
      </Button>
    </LoginShell>
  );
};

const LoginShell = ({
  children,
  onSubmit,
}: {
  children: ReactNode;
  onSubmit: () => void;
}) => (
  <AuthFormShell
    header={
      <Stack sx={{ alignItems: 'flex-end', mb: 2, width: authFormWidth }}>
        <LanguageSwitcher />
      </Stack>
    }
    onSubmit={onSubmit}
  >
    {children}
  </AuthFormShell>
);
