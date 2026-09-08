import { Button, Paper, Stack, TextField, Typography } from '@mui/material';
import { useForm } from '@tanstack/react-form';
import { useQueryClient } from '@tanstack/react-query';
import { Link, useNavigate } from '@tanstack/react-router';
import { type ReactNode, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { LanguageSwitcher } from '../../components/LanguageSwitcher';
import { discoverWorkspace, login } from './api';
import { authQueryKeys } from './query-keys';

const devLoginDefaults = import.meta.env.DEV
  ? {
      email: 'owner@example.test',
      loginIdentifier: 'default.local',
      password: 'test',
    }
  : { email: '', loginIdentifier: '', password: '' };

export const WorkspaceLoginPage = () => {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const [error, setError] = useState<string>();
  const form = useForm({
    defaultValues: {
      loginIdentifier: devLoginDefaults.loginIdentifier,
    },
    onSubmit: async ({ value }) => {
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
    },
  });
  return (
    <LoginShell onSubmit={() => form.handleSubmit()}>
      <Typography variant="h5">{t('auth.signIn')}</Typography>
      <Typography>{t('auth.workspacePrompt')}</Typography>
      <form.Field name="loginIdentifier">
        {(field) => (
          <TextField
            autoComplete="organization"
            label={t('auth.workspace')}
            onChange={(event) => field.handleChange(event.target.value)}
            value={field.state.value}
          />
        )}
      </form.Field>
      {error && <Typography color="error">{error}</Typography>}
      <Button type="submit" variant="contained">
        {t('auth.continue')}
      </Button>
    </LoginShell>
  );
};

export const PasswordLoginPage = ({ identifier }: { identifier: string }) => {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const [error, setError] = useState<string>();
  const form = useForm({
    defaultValues: {
      email: devLoginDefaults.email,
      password: devLoginDefaults.password,
    },
    onSubmit: async ({ value }) => {
      try {
        await login(identifier, value.email, value.password);
        await queryClient.invalidateQueries({
          queryKey: authQueryKeys.session(),
        });
        const returnTo = sessionStorage.getItem('catalog.return-to');
        sessionStorage.removeItem('catalog.return-to');
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
    },
  });
  return (
    <LoginShell onSubmit={() => form.handleSubmit()}>
      <Typography variant="h5">{t('auth.signInTo', { identifier })}</Typography>
      <Button component={Link} to="/login" variant="text">
        {t('auth.changeWorkspace')}
      </Button>
      <form.Field name="email">
        {(field) => (
          <TextField
            autoComplete="username"
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
            label={t('auth.password')}
            onChange={(event) => field.handleChange(event.target.value)}
            type="password"
            value={field.state.value}
          />
        )}
      </form.Field>
      {error && <Typography color="error">{error}</Typography>}
      <Button component={Link} to="/password-reset" variant="text">
        {t('auth.forgotPassword')}
      </Button>
      <Button type="submit" variant="contained">
        Sign in
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
  <Stack
    sx={{ alignItems: 'center', justifyContent: 'center', minHeight: '100dvh' }}
  >
    <Stack sx={{ alignItems: 'flex-end', mb: 2, width: 360 }}>
      <LanguageSwitcher />
    </Stack>
    <Paper
      component="form"
      onSubmit={(event) => {
        event.preventDefault();
        onSubmit();
      }}
      sx={{ p: 4, width: 360 }}
    >
      <Stack spacing={2}>{children}</Stack>
    </Paper>
  </Stack>
);
