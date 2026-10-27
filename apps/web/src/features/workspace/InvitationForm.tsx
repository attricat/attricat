import { useForm } from '@tanstack/react-form';
import { useQueryClient } from '@tanstack/react-query';
import {
  Alert,
  Button,
  Paper,
  Stack,
  TextField,
  Typography,
} from '@mui/material';
import { useTranslation } from 'react-i18next';
import { useExclusiveMutation } from '../../components/useExclusiveMutation';
import { futureZonedInstant } from '../../time/instantFormat';
import { useTimeZone } from '../../time/useInstantFormat';
import {
  createInvitation,
  createWorkspaceUser,
  ensureActiveScopeTarget,
  selectedScopeTarget,
  type ScopeType,
} from './api';
import { scopeTypes } from './constants';
import { ExpiryField, RoleSelectField } from './InvitationFormFields';
import { workspaceQueryKeys } from './queryKeys';
import { ScopeFields } from './ScopeFields';

type InvitationValues = {
  email: string;
  display_name: string;
  role_id: string;
  scope_type: ScopeType;
  scope_target_id: string;
  expires_at: string;
};

/**
 * Invites someone to the workspace with a role. With `createUser` the
 * account is created first, optionally named; otherwise the email must
 * belong to an existing user.
 */
export const InvitationForm = ({
  createUser = false,
  onCreated,
  onError,
  roles,
  workspaceId,
}: {
  createUser?: boolean;
  onCreated: () => unknown;
  onError: (error?: Error) => void;
  roles?: { code: string; id: string }[];
  workspaceId?: string;
}) => {
  const { t } = useTranslation();
  const client = useQueryClient();
  const timeZone = useTimeZone();
  const invite = useExclusiveMutation({
    mutationFn: async (value: InvitationValues) => {
      const expiresAt = futureZonedInstant(value.expires_at, timeZone);
      if (!expiresAt) throw new Error(t('workspace.invitationExpiry'));
      const input = {
        email: value.email,
        ...ensureActiveScopeTarget(
          {
            role_id: value.role_id,
            scope_type: value.scope_type,
            scope_target_id: selectedScopeTarget(
              value.scope_type,
              value.scope_target_id,
              workspaceId,
            ),
          },
          workspaceId,
          client.getQueryData(
            workspaceQueryKeys.grantTargets(value.scope_type),
          ),
        ),
        expires_at: expiresAt.toISOString(),
      };
      if (createUser)
        await createWorkspaceUser({
          ...input,
          display_name: value.display_name || undefined,
        });
      else await createInvitation(input);
    },
    onMutate: () => onError(undefined),
    onSuccess: () => {
      void onCreated();
      form.reset();
    },
    onError: (reason) => onError(reason),
  });
  const form = useForm({
    defaultValues: {
      email: '',
      display_name: '',
      role_id: '',
      scope_type: scopeTypes.workspace,
      scope_target_id: workspaceId ?? '',
      expires_at: '',
    } as InvitationValues,
    onSubmit: ({ value }) => invite.submit(value),
  });
  const title = createUser
    ? t('workspace.createUserInvite')
    : t('workspace.inviteExisting');
  return (
    <Paper
      component="form"
      onSubmit={(event) => {
        event.preventDefault();
        form.handleSubmit();
      }}
      sx={{ p: 2 }}
    >
      <Stack spacing={2}>
        <Typography variant="h6">{title}</Typography>
        {createUser && (
          <Alert severity="info">{t('workspace.inviteEmailInfo')}</Alert>
        )}
        <form.Field name="email">
          {(field) => (
            <TextField
              required
              label={t('workspace.email')}
              onChange={(event) => field.handleChange(event.target.value)}
              type="email"
              value={field.state.value}
            />
          )}
        </form.Field>
        {createUser && (
          <form.Field name="display_name">
            {(field) => (
              <TextField
                label={t('workspace.displayNameOptional')}
                onChange={(event) => field.handleChange(event.target.value)}
                value={field.state.value}
              />
            )}
          </form.Field>
        )}
        <form.Field name="role_id">
          {(field) => (
            <RoleSelectField
              onChange={field.handleChange}
              roles={roles}
              value={field.state.value}
            />
          )}
        </form.Field>
        <form.Subscribe selector={(state) => state.values}>
          {(values) => (
            <ScopeFields
              onScopeChange={(scope) => form.setFieldValue('scope_type', scope)}
              onScopeTargetChange={(scopeTargetId) =>
                form.setFieldValue('scope_target_id', scopeTargetId)
              }
              scope={values.scope_type}
              scopeTargetId={values.scope_target_id}
              workspaceId={workspaceId}
            />
          )}
        </form.Subscribe>
        <form.Field name="expires_at">
          {(field) => (
            <ExpiryField
              onChange={field.handleChange}
              timeZone={timeZone}
              value={field.state.value}
            />
          )}
        </form.Field>
        <Button disabled={invite.isPending} type="submit" variant="contained">
          {createUser
            ? t('workspace.createUserInvite')
            : t('workspace.createInvitation')}
        </Button>
      </Stack>
    </Paper>
  );
};
