import { useForm } from '@tanstack/react-form';
import { useQueryClient } from '@tanstack/react-query';
import { Button, Paper, Stack, TextField, Typography } from '@mui/material';
import { useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
  createInvitation,
  ensureActiveScopeTarget,
  selectedScopeTarget,
} from './api';
import { workspaceScope } from './constants';
import { ExpiryField, RoleSelectField } from './InvitationFormFields';
import { parseFutureExpiry } from './invitationExpiry';
import { workspaceQueryKeys } from './queryKeys';
import { ScopeFields } from './ScopeFields';

export const InviteExistingUserForm = ({
  onCreated,
  onError,
  roles,
  workspaceId,
}: {
  onCreated: () => unknown;
  onError: (message?: string) => void;
  roles?: { code: string; id: string }[];
  workspaceId?: string;
}) => {
  const { t } = useTranslation();
  const client = useQueryClient();
  const inviting = useRef(false);
  const [isInviting, setIsInviting] = useState(false);
  const form = useForm({
    defaultValues: {
      email: '',
      role_id: '',
      scope_type: workspaceScope,
      scope_target_id: workspaceId ?? '',
      expires_at: '',
    },
    onSubmit: async ({ value }) => {
      if (inviting.current) return;
      inviting.current = true;
      setIsInviting(true);
      onError(undefined);
      try {
        const expiresAt = parseFutureExpiry(value.expires_at);
        const input = ensureActiveScopeTarget(
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
        );
        await createInvitation({
          email: value.email,
          ...input,
          expires_at: expiresAt.toISOString(),
        });
        void onCreated();
        form.reset();
      } catch (reason) {
        onError(
          reason instanceof Error
            ? reason.message
            : t('workspace.createInvitationFailed'),
        );
      } finally {
        inviting.current = false;
        setIsInviting(false);
      }
    },
  });
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
        <Typography variant="h6">{t('workspace.inviteExisting')}</Typography>
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
              value={field.state.value}
            />
          )}
        </form.Field>
        <Button disabled={isInviting} type="submit" variant="contained">
          {t('workspace.createInvitation')}
        </Button>
      </Stack>
    </Paper>
  );
};
