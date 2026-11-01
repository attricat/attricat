import { Button, Divider, TextField, Typography } from '@mui/material';
import { useForm } from '@tanstack/react-form';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';
import { useTranslation } from 'react-i18next';
import { ApiErrorAlert } from '../../components/ApiErrorAlert';
import { AuthFormShell } from '../auth/AuthFormShell';
import { getInvitationOptions } from '../auth/api';
import { authQueryKeys } from '../auth/queryKeys';
import { SingleSignOnButton } from '../auth/SingleSignOnButton';
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
  const queryClient = useQueryClient();
  const options = useQuery({
    queryKey: authQueryKeys.invitationOptions(initialInvitationSecret),
    queryFn: () => getInvitationOptions(initialInvitationSecret),
    enabled: Boolean(initialInvitationSecret),
    retry: false,
  });
  const setup = useMutation({
    meta: { toast: false },
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
    onSuccess: async () => {
      // Completing onboarding signs the user in, but this page already cached
      // the signed-out session, which would send them to sign in again.
      await queryClient.refetchQueries({ queryKey: authQueryKeys.session() });
      await navigate({ to: '/' });
    },
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
      <Typography component="h1" variant="h5">
        {t('workspace.setupTitle')}
      </Typography>
      {options.data?.display_name && (
        <>
          <SingleSignOnButton
            displayName={options.data.display_name}
            invitationSecret={initialInvitationSecret}
            loginIdentifier={options.data.login_identifier}
          />
          <Divider>{t('auth.or')}</Divider>
        </>
      )}
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
