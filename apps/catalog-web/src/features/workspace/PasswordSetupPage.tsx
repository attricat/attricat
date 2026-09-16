import { Alert, Button, Stack, TextField, Typography } from '@mui/material';
import { useForm } from '@tanstack/react-form';
import { useNavigate } from '@tanstack/react-router';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { NarrowPage } from '../../components/CenteredPage';
import { completeOnboarding } from './api';

type Notice = { severity: 'error' | 'success'; text: string };

const minimumPasswordLength = 5;

export const PasswordSetupPage = ({
  initialInvitationSecret = '',
  initialOnboardingSecret = '',
}: {
  initialInvitationSecret?: string;
  initialOnboardingSecret?: string;
}) => {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const [notice, setNotice] = useState<Notice>();
  const form = useForm({
    defaultValues: {
      confirmPassword: '',
      invitationSecret: initialInvitationSecret,
      onboardingSecret: initialOnboardingSecret,
      password: '',
    },
    onSubmit: async ({ value }) => {
      try {
        if (value.password.length < minimumPasswordLength)
          throw new Error(t('workspace.passwordTooShort'));
        if (value.password !== value.confirmPassword)
          throw new Error(t('workspace.passwordMismatch'));
        await completeOnboarding({
          invitation_secret: value.invitationSecret,
          onboarding_secret: value.onboardingSecret,
          password: value.password,
        });
        await navigate({ to: '/' });
      } catch (reason) {
        setNotice({
          severity: 'error',
          text:
            reason instanceof Error
              ? reason.message
              : t('workspace.onboardingFailed'),
        });
      }
    },
  });
  return (
    <NarrowPage>
      <Typography variant="h4">{t('workspace.setupTitle')}</Typography>
      <Stack
        component="form"
        onSubmit={(event) => {
          event.preventDefault();
          form.handleSubmit();
        }}
        spacing={2}
        sx={{ mt: 3 }}
      >
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
        {notice && <Alert severity={notice.severity}>{notice.text}</Alert>}
        <Button type="submit" variant="contained">
          {t('workspace.setPassword')}
        </Button>
      </Stack>
    </NarrowPage>
  );
};
