import { Alert, Button, Stack, TextField, Typography } from '@mui/material';
import { useForm } from '@tanstack/react-form';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';
import { useTranslation } from 'react-i18next';
import { ApiErrorAlert } from '../../components/ApiErrorAlert';
import { NarrowPage } from '../../components/CenteredPage';
import { useInitialFocus } from '../../components/initialFocus';
import { PageHeader } from '../../components/PageHeader';
import { InvitationIcon } from '../../components/systemIcons';
import { RouterButton } from '../../components/RouterLink';
import { getInvitationOptions, login } from '../auth/api';
import { sessionOptions } from '../auth/queryOptions';
import { authQueryKeys } from '../auth/queryKeys';
import { SingleSignOnButton } from '../auth/SingleSignOnButton';
import { acceptInvitation } from './api';

/** Accepts with an existing account's password, which also signs in. */
const PasswordInvitationForm = ({
  identifier,
  secret,
}: {
  identifier: string;
  secret: string;
}) => {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const signIn = useMutation({
    meta: { toast: false },
    mutationFn: ({ email, password }: { email: string; password: string }) =>
      login(identifier, email, password, secret),
    onSuccess: async (session) => {
      queryClient.setQueryData(authQueryKeys.session(), session);
      await navigate({ replace: true, to: '/' });
    },
  });
  const form = useForm({
    defaultValues: { email: '', password: '' },
    onSubmit: ({ value }) => signIn.mutate(value),
  });
  return (
    <Stack
      component="form"
      onSubmit={(event) => {
        event.preventDefault();
        form.handleSubmit();
      }}
      spacing={2}
    >
      <Typography color="text.secondary" variant="body2">
        {t('workspace.acceptWithPasswordHint')}
      </Typography>
      <form.Field name="email">
        {(field) => (
          <TextField
            autoComplete="username"
            label={t('auth.email')}
            onChange={(event) => field.handleChange(event.target.value)}
            type="email"
            value={field.state.value}
          />
        )}
      </form.Field>
      <form.Field name="password">
        {(field) => (
          <TextField
            autoComplete="current-password"
            label={t('auth.password')}
            onChange={(event) => field.handleChange(event.target.value)}
            type="password"
            value={field.state.value}
          />
        )}
      </form.Field>
      <ApiErrorAlert error={signIn.error} />
      <Button disabled={signIn.isPending} type="submit" variant="contained">
        {t('workspace.signInToAccept')}
      </Button>
      <RouterButton to="/password-reset" variant="text">
        {t('auth.forgotPassword')}
      </RouterButton>
    </Stack>
  );
};

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
      {identifier && options.data?.password_allowed && (
        <PasswordInvitationForm identifier={identifier} secret={secret} />
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
  const queryClient = useQueryClient();
  const session = useQuery(sessionOptions());
  const formRef = useInitialFocus<HTMLFormElement>();
  const accept = useMutation({
    meta: { toast: false },
    mutationFn: acceptInvitation,
    onSuccess: async () => {
      form.reset({ secret: '' });
      // The new grant changes what the session and every page may show.
      await queryClient.invalidateQueries();
    },
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
