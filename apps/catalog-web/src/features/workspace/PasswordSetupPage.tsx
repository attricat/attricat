import {
  Alert,
  Box,
  Button,
  Stack,
  TextField,
  Typography,
} from '@mui/material';
import { useNavigate } from '@tanstack/react-router';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { completeOnboarding } from './api';

type Notice = { severity: 'error' | 'success'; text: string };

export const PasswordSetupPage = ({
  initialInvitationSecret = '',
  initialOnboardingSecret = '',
}: {
  initialInvitationSecret?: string;
  initialOnboardingSecret?: string;
}) => {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const [invitationSecret, setInvitationSecret] = useState(
    initialInvitationSecret,
  );
  const [onboardingSecret, setOnboardingSecret] = useState(
    initialOnboardingSecret,
  );
  const [password, setPassword] = useState('');
  const [confirmPassword, setConfirmPassword] = useState('');
  const [notice, setNotice] = useState<Notice>();
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
      setNotice({
        severity: 'error',
        text:
          reason instanceof Error
            ? reason.message
            : t('workspace.onboardingFailed'),
      });
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
        {notice && <Alert severity={notice.severity}>{notice.text}</Alert>}
        <Button onClick={submit} variant="contained">
          {t('workspace.setPassword')}
        </Button>
      </Stack>
    </Box>
  );
};
