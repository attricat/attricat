import { useForm } from '@tanstack/react-form';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import {
  Alert,
  Button,
  List,
  ListItem,
  ListItemText,
  MenuItem,
  Paper,
  Stack,
  TextField,
  Typography,
} from '@mui/material';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
  createInvitation,
  createWorkspaceUser,
  ensureActiveScopeTarget,
  listAssignableRoles,
  listInvitations,
  revokeInvitation,
  selectedScopeTarget,
  type ScopeType,
} from './api';
import { workspaceQueryKeys } from './query-keys';
import { ScopeFields } from './ScopeFields';

export const WorkspaceInvitationsSection = ({
  canManage,
  workspaceId,
}: {
  canManage: boolean;
  workspaceId?: string;
}) => {
  const { t } = useTranslation();
  const client = useQueryClient();
  const [error, setError] = useState<string>();
  const invitations = useQuery({
    enabled: canManage,
    queryKey: workspaceQueryKeys.invitations(),
    queryFn: listInvitations,
  });
  const roles = useQuery({
    enabled: canManage,
    queryKey: workspaceQueryKeys.assignableRoles(),
    queryFn: listAssignableRoles,
  });
  const refresh = () =>
    client.invalidateQueries({ queryKey: workspaceQueryKeys.invitations() });
  const form = useForm({
    defaultValues: {
      email: '',
      role_id: '',
      scope_type: 'workspace' as ScopeType,
      scope_target_id: workspaceId ?? '',
      expires_at: '',
    },
    onSubmit: async ({ value }) => {
      try {
        const expiresAt = new Date(value.expires_at);
        if (Number.isNaN(expiresAt.getTime()) || expiresAt <= new Date()) {
          throw new Error(t('workspace.invitationExpiry'));
        }
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
        refresh();
      } catch (reason) {
        setError(
          reason instanceof Error
            ? reason.message
            : t('workspace.createInvitationFailed'),
        );
      }
    },
  });
  const userForm = useForm({
    defaultValues: {
      email: '',
      display_name: '',
      role_id: '',
      scope_type: 'workspace' as ScopeType,
      scope_target_id: workspaceId ?? '',
      expires_at: '',
    },
    onSubmit: async ({ value }) => {
      try {
        const expiresAt = new Date(value.expires_at);
        if (Number.isNaN(expiresAt.getTime()) || expiresAt <= new Date())
          throw new Error(t('workspace.invitationExpiry'));
        const input = ensureActiveScopeTarget(
          {
            role_id: value.role_id,
            scope_type: value.scope_type,
            scope_target_id:
              value.scope_type === 'workspace'
                ? (workspaceId ?? '')
                : value.scope_target_id,
          },
          workspaceId,
          client.getQueryData(
            workspaceQueryKeys.grantTargets(value.scope_type),
          ),
        );
        await createWorkspaceUser({
          email: value.email,
          display_name: value.display_name || undefined,
          ...input,
          expires_at: expiresAt.toISOString(),
        });
        refresh();
      } catch (reason) {
        setError(
          reason instanceof Error
            ? reason.message
            : t('workspace.createUserFailed'),
        );
      }
    },
  });
  if (!canManage) {
    return (
      <Alert severity="error">{t('workspace.notAuthorizedInvitations')}</Alert>
    );
  }
  return (
    <Stack spacing={2} sx={{ mt: 3 }}>
      {error && <Alert severity="error">{error}</Alert>}
      {invitations.isError && (
        <Alert severity="error">{invitations.error.message}</Alert>
      )}
      {roles.isError && <Alert severity="error">{roles.error.message}</Alert>}
      <Paper>
        <List>
          {invitations.data?.map((item) => (
            <ListItem
              divider
              key={item.id}
              secondaryAction={
                !item.revoked_at &&
                !item.accepted_at && (
                  <Button
                    color="error"
                    onClick={() =>
                      revokeInvitation(item.id)
                        .then(refresh)
                        .catch((e) => setError(e.message))
                    }
                  >
                    Revoke
                  </Button>
                )
              }
            >
              <ListItemText
                primary={item.invitee_email}
                secondary={`${item.role_code} · ${t('workspace.expires', { date: new Date(item.expires_at).toLocaleString() })}`}
              />
            </ListItem>
          ))}
        </List>
      </Paper>
      <Paper
        component="form"
        onSubmit={(event) => {
          event.preventDefault();
          userForm.handleSubmit();
        }}
        sx={{ p: 2 }}
      >
        <Stack spacing={2}>
          <Typography variant="h6">
            {t('workspace.createUserInvite')}
          </Typography>
          <Alert severity="info">{t('workspace.inviteEmailInfo')}</Alert>
          <userForm.Field name="email">
            {(field) => (
              <TextField
                label={t('workspace.email')}
                type="email"
                value={field.state.value}
                onChange={(event) => field.handleChange(event.target.value)}
              />
            )}
          </userForm.Field>
          <userForm.Field name="display_name">
            {(field) => (
              <TextField
                label={t('workspace.displayNameOptional')}
                value={field.state.value}
                onChange={(event) => field.handleChange(event.target.value)}
              />
            )}
          </userForm.Field>
          <userForm.Field name="role_id">
            {(field) => (
              <TextField
                label={t('workspace.role')}
                select
                value={field.state.value}
                onChange={(event) => field.handleChange(event.target.value)}
              >
                {roles.data?.map((role) => (
                  <MenuItem key={role.id} value={role.id}>
                    {role.code}
                  </MenuItem>
                ))}
              </TextField>
            )}
          </userForm.Field>
          <userForm.Subscribe selector={(state) => state.values}>
            {(values) => (
              <ScopeFields
                onScopeChange={(scope) =>
                  userForm.setFieldValue('scope_type', scope)
                }
                onScopeTargetChange={(scopeTargetId) =>
                  userForm.setFieldValue('scope_target_id', scopeTargetId)
                }
                scope={values.scope_type}
                scopeTargetId={values.scope_target_id}
                workspaceId={workspaceId}
              />
            )}
          </userForm.Subscribe>
          <userForm.Field name="expires_at">
            {(field) => (
              <TextField
                label={t('workspace.expiresAt')}
                type="datetime-local"
                slotProps={{ inputLabel: { shrink: true } }}
                value={field.state.value}
                onChange={(event) => field.handleChange(event.target.value)}
              />
            )}
          </userForm.Field>
          <Button type="submit" variant="contained">
            Create user and invite
          </Button>
        </Stack>
      </Paper>
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
                label={t('workspace.email')}
                onChange={(event) => field.handleChange(event.target.value)}
                type="email"
                value={field.state.value}
              />
            )}
          </form.Field>
          <form.Field name="role_id">
            {(field) => (
              <TextField
                label={t('workspace.role')}
                onChange={(event) => field.handleChange(event.target.value)}
                select
                value={field.state.value}
              >
                {roles.data?.map((role) => (
                  <MenuItem key={role.id} value={role.id}>
                    {role.code}
                  </MenuItem>
                ))}
              </TextField>
            )}
          </form.Field>
          <form.Subscribe selector={(state) => state.values}>
            {(values) => (
              <ScopeFields
                onScopeChange={(scope) =>
                  form.setFieldValue('scope_type', scope)
                }
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
              <TextField
                slotProps={{ inputLabel: { shrink: true } }}
                label={t('workspace.expiresAt')}
                onChange={(event) => field.handleChange(event.target.value)}
                type="datetime-local"
                value={field.state.value}
              />
            )}
          </form.Field>
          <Button type="submit" variant="contained">
            Create invitation
          </Button>
        </Stack>
      </Paper>
    </Stack>
  );
};
