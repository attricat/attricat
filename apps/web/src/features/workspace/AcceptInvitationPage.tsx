import { Alert, Button, Stack, TextField, Typography } from '@mui/material';
import { useForm } from '@tanstack/react-form';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { errorMessage } from '../../api/errors';
import { NarrowPage } from '../../components/CenteredPage';
import { acceptInvitation } from './api';
import { useInitialFocus } from '../../components/initialFocus';

type Notice = { severity: 'error' | 'success'; text: string };

export const AcceptInvitationPage = ({
  secret: initialSecret = '',
}: {
  secret?: string;
}) => {
  const { t } = useTranslation();
  const formRef = useInitialFocus<HTMLFormElement>();
  const [notice, setNotice] = useState<Notice>();
  const form = useForm({
    defaultValues: { secret: initialSecret },
    onSubmit: async ({ value }) => {
      try {
        await acceptInvitation(value.secret);
        form.reset({ secret: '' });
        setNotice({
          severity: 'success',
          text: t('workspace.invitationAccepted'),
        });
      } catch (reason) {
        setNotice({
          severity: 'error',
          text:
            reason instanceof Error
              ? errorMessage(reason)
              : t('workspace.acceptInvitationFailed'),
        });
      }
    },
  });
  return (
    <NarrowPage>
      <Typography variant="h4">{t('workspace.acceptTitle')}</Typography>
      <Stack
        ref={formRef}
        component="form"
        onSubmit={(event) => {
          event.preventDefault();
          form.handleSubmit();
        }}
        spacing={2}
        sx={{ mt: 3 }}
      >
        <form.Field name="secret">
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
        {notice && <Alert severity={notice.severity}>{notice.text}</Alert>}
        <Button type="submit" variant="contained">
          {t('workspace.acceptInvitation')}
        </Button>
      </Stack>
    </NarrowPage>
  );
};
