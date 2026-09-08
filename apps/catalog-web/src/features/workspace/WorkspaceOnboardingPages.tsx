import {
  Button,
  Alert,
  Box,
  Stack,
  TextField,
  Typography,
} from '@mui/material';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useNavigate } from '@tanstack/react-router';
import { acceptInvitation, completeOnboarding } from './api';

export const PasswordSetupPage = () => {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const [invitationSecret, setInvitationSecret] = useState(
    () =>
      new URLSearchParams(window.location.search).get('invitation_secret') ??
      '',
  );
  const [onboardingSecret, setOnboardingSecret] = useState(
    () =>
      new URLSearchParams(window.location.search).get('onboarding_secret') ??
      '',
  );
  const [password, setPassword] = useState('');
  const [confirmPassword, setConfirmPassword] = useState('');
  const [message, setMessage] = useState<string>();
  const submit = async () => {
    try {
      if (password.length < 12)
        throw new Error(t('workspace.passwordTooShort'));
      if (password !== confirmPassword)
        throw new Error(t('workspace.passwordMismatch'));
      await completeOnboarding({
        invitation_secret: invitationSecret,
        onboarding_secret: onboardingSecret,
        password,
      });
      await navigate({ to: '/' });
    } catch (reason) {
      setMessage(
        reason instanceof Error
          ? reason.message
          : t('workspace.onboardingFailed'),
      );
    }
  };
  return (
    <Box sx={{ maxWidth: 500, mx: 'auto', p: 3 }}>
      <Typography variant="h4">{t('workspace.setupTitle')}</Typography>
      <Stack spacing={2} sx={{ mt: 3 }}>
        <TextField
          autoComplete="off"
          label={t('workspace.invitationSecret')}
          onChange={(event) => setInvitationSecret(event.target.value)}
          type="password"
          value={invitationSecret}
        />
        <TextField
          autoComplete="off"
          label={t('workspace.passwordSetupSecret')}
          onChange={(event) => setOnboardingSecret(event.target.value)}
          type="password"
          value={onboardingSecret}
        />
        <TextField
          autoComplete="new-password"
          label={t('workspace.password')}
          onChange={(event) => setPassword(event.target.value)}
          type="password"
          value={password}
        />
        <TextField
          autoComplete="new-password"
          label={t('workspace.confirmPassword')}
          onChange={(event) => setConfirmPassword(event.target.value)}
          type="password"
          value={confirmPassword}
        />
        {message && (
          <Alert
            severity={message.startsWith('Password set') ? 'success' : 'error'}
          >
            {message}
          </Alert>
        )}
        <Button onClick={submit} variant="contained">
          Set password and join workspace
        </Button>
      </Stack>
    </Box>
  );
};

export const AcceptInvitationPage = () => {
  const { t } = useTranslation();
  const [secret, setSecret] = useState(
    () => new URLSearchParams(window.location.search).get('secret') ?? '',
  );
  const [message, setMessage] = useState<string>();
  const submit = async () => {
    try {
      await acceptInvitation(secret);
      setSecret('');
      setMessage(t('workspace.invitationAccepted'));
    } catch (reason) {
      setMessage(
        reason instanceof Error
          ? reason.message
          : t('workspace.acceptInvitationFailed'),
      );
    }
  };
  return (
    <Box sx={{ maxWidth: 500, mx: 'auto', p: 3 }}>
      <Typography variant="h4">{t('workspace.acceptTitle')}</Typography>
      <Stack spacing={2} sx={{ mt: 3 }}>
        <TextField
          autoComplete="off"
          label={t('workspace.invitationSecret')}
          onChange={(event) => setSecret(event.target.value)}
          type="password"
          value={secret}
        />
        {message && (
          <Alert
            severity={
              message.startsWith('Invitation accepted') ? 'success' : 'error'
            }
          >
            {message}
          </Alert>
        )}
        <Button onClick={submit} variant="contained">
          Accept invitation
        </Button>
      </Stack>
    </Box>
  );
};
