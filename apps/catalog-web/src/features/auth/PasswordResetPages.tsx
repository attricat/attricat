import { Button, Paper, Stack, TextField, Typography } from '@mui/material';
import { useForm } from '@tanstack/react-form';
import { Link, useNavigate } from '@tanstack/react-router';
import { type ReactNode, useState } from 'react';
import { confirmPasswordReset, requestPasswordReset } from './api';

export const PasswordResetRequestPage = () => {
  const [error, setError] = useState<string>();
  const [submitted, setSubmitted] = useState(false);
  const form = useForm({
    defaultValues: { email: '' },
    onSubmit: async ({ value }) => {
      try {
        await requestPasswordReset(value.email);
        setSubmitted(true);
      } catch (reason) {
        setError(
          reason instanceof Error
            ? reason.message
            : 'Unable to request a password reset',
        );
      }
    },
  });
  return (
    <PasswordResetShell onSubmit={() => form.handleSubmit()}>
      <Typography variant="h5">Reset your password</Typography>
      {submitted ? (
        <Typography>
          If an eligible account uses that email address, a reset link has been
          sent.
        </Typography>
      ) : (
        <>
          <Typography>
            Enter your email address to receive a password reset link.
          </Typography>
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
          {error && <Typography color="error">{error}</Typography>}
          <Button type="submit" variant="contained">
            Send reset link
          </Button>
        </>
      )}
      <Button component={Link} to="/login" variant="text">
        Back to sign in
      </Button>
    </PasswordResetShell>
  );
};

export const PasswordResetConfirmationPage = ({
  token,
}: {
  token?: string;
}) => {
  const navigate = useNavigate();
  const [error, setError] = useState<string>();
  const [submitted, setSubmitted] = useState(false);
  const form = useForm({
    defaultValues: { password: '' },
    onSubmit: async ({ value }) => {
      if (!token) {
        setError('This password reset link is invalid or expired');
        return;
      }
      try {
        await confirmPasswordReset(token, value.password);
        setSubmitted(true);
      } catch (reason) {
        setError(
          reason instanceof Error ? reason.message : 'Unable to reset password',
        );
      }
    },
  });
  return (
    <PasswordResetShell onSubmit={() => form.handleSubmit()}>
      <Typography variant="h5">Choose a new password</Typography>
      {submitted ? (
        <>
          <Typography>
            Your password has been reset. Sign in with your new password.
          </Typography>
          <Button
            onClick={() => navigate({ to: '/login' })}
            variant="contained"
          >
            Go to sign in
          </Button>
        </>
      ) : (
        <>
          <form.Field name="password">
            {(field) => (
              <TextField
                autoComplete="new-password"
                label="New password"
                onChange={(event) => field.handleChange(event.target.value)}
                type="password"
                value={field.state.value}
              />
            )}
          </form.Field>
          {error && <Typography color="error">{error}</Typography>}
          <Button type="submit" variant="contained">
            Reset password
          </Button>
        </>
      )}
    </PasswordResetShell>
  );
};

const PasswordResetShell = ({
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
