import { Button, Paper, Stack, TextField, Typography } from '@mui/material';
import { useForm } from '@tanstack/react-form';
import { Link, useNavigate } from '@tanstack/react-router';
import { type ReactNode, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { confirmPasswordReset, requestPasswordReset } from './api';

export const PasswordResetRequestPage = () => {
  const { t } = useTranslation();
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
            : t('auth.requestResetFailed'),
        );
      }
    },
  });
  return (
    <PasswordResetShell onSubmit={() => form.handleSubmit()}>
      <Typography variant="h5">{t('auth.resetPassword')}</Typography>
      {submitted ? (
        <Typography>{t('auth.resetPasswordSent')}</Typography>
      ) : (
        <>
          <Typography>{t('auth.resetPasswordPrompt')}</Typography>
          <form.Field name="email">
            {(field) => (
              <TextField
                autoComplete="email"
                label={t('auth.email')}
                onChange={(event) => field.handleChange(event.target.value)}
                type="email"
                value={field.state.value}
              />
            )}
          </form.Field>
          {error && <Typography color="error">{error}</Typography>}
          <Button type="submit" variant="contained">
            {t('auth.sendResetLink')}
          </Button>
        </>
      )}
      <Button component={Link} to="/login" variant="text">
        {t('auth.backToSignIn')}
      </Button>
    </PasswordResetShell>
  );
};

export const PasswordResetConfirmationPage = ({
  token,
}: {
  token?: string;
}) => {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const [error, setError] = useState<string>();
  const [submitted, setSubmitted] = useState(false);
  const form = useForm({
    defaultValues: { password: '' },
    onSubmit: async ({ value }) => {
      if (!token) {
        setError(t('auth.invalidResetLink'));
        return;
      }
      try {
        await confirmPasswordReset(token, value.password);
        setSubmitted(true);
      } catch (reason) {
        setError(
          reason instanceof Error
            ? reason.message
            : t('auth.resetPasswordFailed'),
        );
      }
    },
  });
  return (
    <PasswordResetShell onSubmit={() => form.handleSubmit()}>
      <Typography variant="h5">{t('auth.chooseNewPassword')}</Typography>
      {submitted ? (
        <>
          <Typography>{t('auth.passwordResetSuccess')}</Typography>
          <Button
            onClick={() => navigate({ to: '/login' })}
            variant="contained"
          >
            {t('auth.goToSignIn')}
          </Button>
        </>
      ) : (
        <>
          <form.Field name="password">
            {(field) => (
              <TextField
                autoComplete="new-password"
                label={t('auth.newPassword')}
                onChange={(event) => field.handleChange(event.target.value)}
                type="password"
                value={field.state.value}
              />
            )}
          </form.Field>
          {error && <Typography color="error">{error}</Typography>}
          <Button type="submit" variant="contained">
            {t('auth.resetPassword')}
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
