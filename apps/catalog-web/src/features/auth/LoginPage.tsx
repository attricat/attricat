import { Button, Paper, Stack, TextField, Typography } from '@mui/material';
import { useForm } from '@tanstack/react-form';
import { useQueryClient } from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';
import { useState } from 'react';
import { login } from './api';

export const LoginPage = () => {
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const [error, setError] = useState<string>();
  const form = useForm({
    defaultValues: { email: '', password: '' },
    onSubmit: async ({ value }) => {
      try {
        await login(value.email, value.password);
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
    <Stack
      alignItems="center"
      justifyContent="center"
      sx={{ minHeight: '100dvh' }}
    >
      <Paper
        component="form"
        onSubmit={(event) => {
          event.preventDefault();
          form.handleSubmit();
        }}
        sx={{ p: 4, width: 360 }}
      >
        <Stack spacing={2}>
          <Typography variant="h5">Sign in</Typography>
          <form.Field name="email">
            {(field) => (
              <TextField
                autoComplete="email"
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
          <Button type="submit" variant="contained">
            Sign in
          </Button>
        </Stack>
      </Paper>
    </Stack>
  );
};
