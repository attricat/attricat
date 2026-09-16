import { Alert, Button, Stack, TextField, Typography } from '@mui/material';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { NarrowPage } from '../../components/CenteredPage';
import { acceptInvitation } from './api';

type Notice = { severity: 'error' | 'success'; text: string };

export const AcceptInvitationPage = ({
  secret: initialSecret = '',
}: {
  secret?: string;
}) => {
  const { t } = useTranslation();
  const [secret, setSecret] = useState(initialSecret);
  const [notice, setNotice] = useState<Notice>();
  const submit = async () => {
    try {
      await acceptInvitation(secret);
      setSecret('');
      setNotice({
        severity: 'success',
        text: t('workspace.invitationAccepted'),
      });
    } catch (reason) {
      setNotice({
        severity: 'error',
        text:
          reason instanceof Error
            ? reason.message
            : t('workspace.acceptInvitationFailed'),
      });
    }
  };
  return (
    <NarrowPage>
      <Typography variant="h4">{t('workspace.acceptTitle')}</Typography>
      <Stack spacing={2} sx={{ mt: 3 }}>
        <TextField
          autoComplete="off"
          label={t('workspace.invitationSecret')}
          onChange={(event) => setSecret(event.target.value)}
          type="password"
          value={secret}
        />
        {notice && <Alert severity={notice.severity}>{notice.text}</Alert>}
        <Button onClick={submit} variant="contained">
          {t('workspace.acceptInvitation')}
        </Button>
      </Stack>
    </NarrowPage>
  );
};
