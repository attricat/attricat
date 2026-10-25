import { Alert, Button, Stack, TextField } from '@mui/material';
import { useForm } from '@tanstack/react-form';
import { useMutation } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import { ApiErrorAlert } from '../../components/ApiErrorAlert';
import { NarrowPage } from '../../components/CenteredPage';
import { useInitialFocus } from '../../components/initialFocus';
import { PageHeader } from '../../components/PageHeader';
import { InvitationIcon } from '../../components/systemIcons';
import { acceptInvitation } from './api';

export const AcceptInvitationPage = ({
  secret: initialSecret = '',
}: {
  secret?: string;
}) => {
  const { t } = useTranslation();
  const formRef = useInitialFocus<HTMLFormElement>();
  const accept = useMutation({
    mutationFn: acceptInvitation,
    onSuccess: () => form.reset({ secret: '' }),
  });
  const form = useForm({
    defaultValues: { secret: initialSecret },
    onSubmit: ({ value }) => accept.mutate(value.secret),
  });
  return (
    <NarrowPage>
      <PageHeader icon={InvitationIcon} title={t('workspace.acceptTitle')} />
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
        <ApiErrorAlert error={accept.error} />
        {accept.isSuccess && (
          <Alert severity="success">{t('workspace.invitationAccepted')}</Alert>
        )}
        <Button disabled={accept.isPending} type="submit" variant="contained">
          {t('workspace.acceptInvitation')}
        </Button>
      </Stack>
    </NarrowPage>
  );
};
