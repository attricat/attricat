import { Button, TextField, Typography } from '@mui/material';
import { useForm } from '@tanstack/react-form';
import { useMutation } from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';
import { useTranslation } from 'react-i18next';
import { ApiErrorAlert } from '../../components/ApiErrorAlert';
import { AuthFormShell } from '../auth/AuthFormShell';
import { completeOnboarding } from './api';

const minimumPasswordLength = 5;

type PasswordSetupValues = {
  confirmPassword: string;
  invitationSecret: string;
  onboardingSecret: string;
  password: string;
};

export const PasswordSetupPage = ({
  initialInvitationSecret = '',
  initialOnboardingSecret = '',
}: {
  initialInvitationSecret?: string;
  initialOnboardingSecret?: string;
}) => {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const setup = useMutation({
    mutationFn: async (value: PasswordSetupValues) => {
      if (value.password.length < minimumPasswordLength)
        throw new Error(t('workspace.passwordTooShort'));
      if (value.password !== value.confirmPassword)
        throw new Error(t('workspace.passwordMismatch'));
      await completeOnboarding({
        invitation_secret: value.invitationSecret,
        onboarding_secret: value.onboardingSecret,
        password: value.password,
      });
    },
    onSuccess: () => navigate({ to: '/' }),
  });
  const form = useForm({
    defaultValues: {
      confirmPassword: '',
      invitationSecret: initialInvitationSecret,
      onboardingSecret: initialOnboardingSecret,
      password: '',
    },
    onSubmit: ({ value }) => setup.mutate(value),
  });
  return (
    <AuthFormShell onSubmit={() => form.handleSubmit()}>
      <Typography variant="h5">{t('workspace.setupTitle')}</Typography>
      <form.Field name="invitationSecret">
        {(field) => (
          <TextField
            autoComplete="off"
            label={t('workspace.invitationSecret')}
            onChange={(event) => field.handleChange(event.target.value)}
            type="password"
            value={field.state.value}
          />
        )}
      </form.Field>
      <form.Field name="onboardingSecret">
        {(field) => (
          <TextField
            autoComplete="off"
            label={t('workspace.passwordSetupSecret')}
            onChange={(event) => field.handleChange(event.target.value)}
            type="password"
            value={field.state.value}
          />
        )}
      </form.Field>
      <form.Field name="password">
        {(field) => (
          <TextField
            autoComplete="new-password"
            label={t('workspace.password')}
            onChange={(event) => field.handleChange(event.target.value)}
            type="password"
            value={field.state.value}
          />
        )}
      </form.Field>
      <form.Field name="confirmPassword">
        {(field) => (
          <TextField
            autoComplete="new-password"
            label={t('workspace.confirmPassword')}
            onChange={(event) => field.handleChange(event.target.value)}
            type="password"
            value={field.state.value}
          />
        )}
      </form.Field>
      <ApiErrorAlert error={setup.error} />
      <Button disabled={setup.isPending} type="submit" variant="contained">
        {t('workspace.setPassword')}
      </Button>
    </AuthFormShell>
  );
};
