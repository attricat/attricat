import { Button, Paper, Stack, TextField, Typography } from '@mui/material';
import { useForm } from '@tanstack/react-form';
import { useQueryClient } from '@tanstack/react-query';
import { Link, useNavigate } from '@tanstack/react-router';
import { type ReactNode, useState } from 'react';
import { discoverWorkspace, login } from './api';

export const WorkspaceLoginPage = () => {
  const navigate = useNavigate();
  const [error, setError] = useState<string>();
  const form = useForm({
    defaultValues: {
      loginIdentifier: import.meta.env.DEV ? 'default.local' : '',
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
          reason instanceof Error ? reason.message : 'Unable to find workspace',
        );
      }
    },
  });
  return (
    <LoginShell onSubmit={() => form.handleSubmit()}>
      <Typography variant="h5">Sign in</Typography>
      <Typography>Enter your workspace identifier.</Typography>
      <form.Field name="loginIdentifier">
        {(field) => (
          <TextField
            autoComplete="organization"
            label="Workspace"
            onChange={(event) => field.handleChange(event.target.value)}
            value={field.state.value}
          />
        )}
      </form.Field>
      {error && <Typography color="error">{error}</Typography>}
      <Button type="submit" variant="contained">
        Continue
      </Button>
    </LoginShell>
  );
};

export const PasswordLoginPage = ({ identifier }: { identifier: string }) => {
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const [error, setError] = useState<string>();
  const form = useForm({
    defaultValues: { email: '', password: '' },
    onSubmit: async ({ value }) => {
      try {
        await login(identifier, value.email, value.password);
        await queryClient.invalidateQueries({ queryKey: ['auth', 'session'] });
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
          reason instanceof Error ? reason.message : 'Unable to sign in',
        );
      }
    },
  });
  return (
    <LoginShell onSubmit={() => form.handleSubmit()}>
      <Typography variant="h5">Sign in to {identifier}</Typography>
      <Button component={Link} to="/login" variant="text">
        Change workspace
      </Button>
      <form.Field name="email">
        {(field) => (
          <TextField
            autoComplete="username"
            label="Email"
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
            label="Password"
            onChange={(event) => field.handleChange(event.target.value)}
            type="password"
            value={field.state.value}
          />
        )}
      </form.Field>
      {error && <Typography color="error">{error}</Typography>}
      <Button component={Link} to="/password-reset" variant="text">
        Forgot password?
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
