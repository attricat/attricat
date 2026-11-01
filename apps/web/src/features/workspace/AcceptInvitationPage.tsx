import { Alert, Button, Stack, TextField } from '@mui/material';
import { useForm } from '@tanstack/react-form';
import { useMutation, useQuery } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import { ApiErrorAlert } from '../../components/ApiErrorAlert';
import { NarrowPage } from '../../components/CenteredPage';
import { useInitialFocus } from '../../components/initialFocus';
import { PageHeader } from '../../components/PageHeader';
import { InvitationIcon } from '../../components/systemIcons';
import { RouterButton } from '../../components/RouterLink';
import { getInvitationOptions } from '../auth/api';
import { sessionOptions } from '../auth/queryOptions';
import { authQueryKeys } from '../auth/queryKeys';
import { rememberReturnLocation } from '../auth/returnLocation';
import { SingleSignOnButton } from '../auth/SingleSignOnButton';
import { acceptInvitation } from './api';

/** The ways to accept for someone who is not signed in. */
const SignedOutInvitation = ({ secret }: { secret: string }) => {
  const { t } = useTranslation();
  const options = useQuery({
    queryKey: authQueryKeys.invitationOptions(secret),
    queryFn: () => getInvitationOptions(secret),
    enabled: Boolean(secret),
    retry: false,
  });
  const identifier = options.data?.login_identifier;
  return (
    <Stack spacing={2} sx={{ mt: 3 }}>
      <ApiErrorAlert error={options.error} />
      {options.isSuccess && !options.data && (
        <Alert severity="error">{t('apiErrors.invitation_invalid')}</Alert>
      )}
      {identifier && options.data?.display_name && (
        <SingleSignOnButton
          displayName={options.data.display_name}
          invitationSecret={secret}
          loginIdentifier={identifier}
        />
      )}
      {identifier && (
        <RouterButton
          onClick={rememberReturnLocation}
          params={{ identifier }}
          to="/login/$identifier"
          variant="outlined"
        >
          {t('workspace.signInToAccept')}
        </RouterButton>
      )}
    </Stack>
  );
};

export const AcceptInvitationPage = ({
  secret: initialSecret = '',
}: {
  secret?: string;
}) => {
  const { t } = useTranslation();
  const session = useQuery(sessionOptions());
  const formRef = useInitialFocus<HTMLFormElement>();
  const accept = useMutation({
    meta: { toast: false },
    mutationFn: acceptInvitation,
    onSuccess: () => form.reset({ secret: '' }),
  });
  const form = useForm({
    defaultValues: { secret: initialSecret },
    onSubmit: ({ value }) => accept.mutate(value.secret),
  });
  if (session.isSuccess && !session.data)
    return (
      <NarrowPage>
        <PageHeader icon={InvitationIcon} title={t('workspace.acceptTitle')} />
        <SignedOutInvitation secret={initialSecret} />
      </NarrowPage>
    );
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
