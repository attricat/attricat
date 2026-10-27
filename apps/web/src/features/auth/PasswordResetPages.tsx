import { Button, TextField, Typography } from '@mui/material';
import { useForm } from '@tanstack/react-form';
import { useNavigate } from '@tanstack/react-router';
import { useTranslation } from 'react-i18next';
import { ArrowLeftIcon } from 'lucide-react';
import { ApiErrorAlert } from '../../components/ApiErrorAlert';
import { useExclusiveMutation } from '../../components/useExclusiveMutation';
import { AuthFormShell } from './AuthFormShell';
import { confirmPasswordReset, requestPasswordReset } from './api';
import { RouterButton } from '../../components/RouterLink';

export const PasswordResetRequestPage = () => {
  const { t } = useTranslation();
  const request = useExclusiveMutation({
    meta: { toast: false },
    mutationFn: (email: string) => requestPasswordReset(email),
  });
  const form = useForm({
    defaultValues: { email: '' },
    onSubmit: ({ value }) => request.submit(value.email),
  });
  return (
    <AuthFormShell onSubmit={() => form.handleSubmit()}>
      <Typography variant="h5">{t('auth.resetPassword')}</Typography>
      {request.isSuccess ? (
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
          <ApiErrorAlert error={request.error} />
          <Button
            disabled={request.isPending}
            type="submit"
            variant="contained"
          >
            {t('auth.sendResetLink')}
          </Button>
        </>
      )}
      <RouterButton startIcon={<ArrowLeftIcon />} to="/login" variant="text">
        {t('auth.backToSignIn')}
      </RouterButton>
    </AuthFormShell>
  );
};

export const PasswordResetConfirmationPage = ({
  token,
}: {
  token?: string;
}) => {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const confirm = useExclusiveMutation({
    meta: { toast: false },
    mutationFn: (password: string) => {
      if (!token) throw new Error(t('auth.invalidResetLink'));
      return confirmPasswordReset(token, password);
    },
  });
  const form = useForm({
    defaultValues: { password: '' },
    onSubmit: ({ value }) => confirm.submit(value.password),
  });
  return (
    <AuthFormShell onSubmit={() => form.handleSubmit()}>
      <Typography variant="h5">{t('auth.chooseNewPassword')}</Typography>
      {confirm.isSuccess ? (
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
          <ApiErrorAlert error={confirm.error} />
          <Button
            disabled={confirm.isPending}
            type="submit"
            variant="contained"
          >
            {t('auth.resetPassword')}
          </Button>
        </>
      )}
    </AuthFormShell>
  );
};
