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
import { useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
  ensureActiveScopeTarget,
  grantMemberRole,
  listAssignableRoles,
  listMembers,
  revokeMemberRole,
  selectedScopeTarget,
  transferOwnership,
  type ScopeType,
} from './api';
import { workspaceQueryKeys } from './queryKeys';
import { ScopeFields } from './ScopeFields';

export const WorkspaceMembersSection = ({
  canManage,
  currentUserId,
  workspaceId,
}: {
  canManage: boolean;
  currentUserId?: string;
  workspaceId?: string;
}) => {
  const { t } = useTranslation();
  const client = useQueryClient();
  const [error, setError] = useState<string>();
  const submitting = useRef(false);
  const [isSubmitting, setIsSubmitting] = useState(false);
  const members = useQuery({
    enabled: canManage,
    queryKey: workspaceQueryKeys.members(),
    queryFn: listMembers,
  });
  const roles = useQuery({
    enabled: canManage,
    queryKey: workspaceQueryKeys.assignableRoles(),
    queryFn: listAssignableRoles,
  });
  const refresh = () =>
    client.invalidateQueries({ queryKey: workspaceQueryKeys.members() });
  const form = useForm({
    defaultValues: {
      member_id: '',
      role_id: '',
      scope_type: 'workspace' as ScopeType,
      scope_target_id: workspaceId ?? '',
    },
    onSubmit: async ({ value }) => {
      if (submitting.current) return;
      submitting.current = true;
      setIsSubmitting(true);
      setError(undefined);
      try {
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
        await grantMemberRole(value.member_id, input);
        void refresh();
        form.reset();
      } catch (reason) {
        setError(
          reason instanceof Error
            ? reason.message
            : t('workspace.grantRoleFailed'),
        );
      } finally {
        submitting.current = false;
        setIsSubmitting(false);
      }
    },
  });
  const isOwner = members.data?.some(
    (member) =>
      member.user_id === currentUserId &&
      member.grants.some(
        (grant) =>
          grant.role_code === 'owner' &&
          grant.scope_type === 'workspace' &&
          grant.scope_target_id === workspaceId,
      ),
  );
  if (!canManage) {
    return (
      <Alert severity="error">{t('workspace.notAuthorizedMembers')}</Alert>
    );
  }
  return (
    <Stack spacing={2} sx={{ mt: 3 }}>
      {error && <Alert severity="error">{error}</Alert>}
      {members.isError && (
        <Alert severity="error">{members.error.message}</Alert>
      )}
      {roles.isError && <Alert severity="error">{roles.error.message}</Alert>}
      <Paper>
        <List>
          {members.data?.map((member) => (
            <ListItem divider key={member.id}>
              <ListItemText
                primary={member.display_name ?? member.email}
                secondary={`${member.email} · ${member.state}${member.grants.length ? ` · ${member.grants.map((grant) => grant.role_code).join(', ')}` : ''}`}
              />
              {member.grants.map((grant) => (
                <Button
                  key={grant.id}
                  onClick={() =>
                    revokeMemberRole(member.id, grant.id)
                      .then(refresh)
                      .catch((e) => setError(e.message))
                  }
                >
                  {t('workspace.revoke', { role: grant.role_code })}
                </Button>
              ))}
              <Stack direction="row" sx={{ gap: 1 }}>
                {isOwner && (
                  <Button
                    onClick={() =>
                      transferOwnership(member.id)
                        .then(refresh)
                        .catch((e) => setError(e.message))
                    }
                  >
                    {t('workspace.transferOwnership')}
                  </Button>
                )}
              </Stack>
            </ListItem>
          ))}
        </List>
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
          <Typography variant="h6">{t('workspace.addRoleGrant')}</Typography>
          <form.Field name="member_id">
            {(field) => (
              <TextField
                label={t('workspace.member')}
                onChange={(event) => field.handleChange(event.target.value)}
                select
                value={field.state.value}
              >
                {members.data
                  ?.filter((member) => member.state === 'active')
                  .map((member) => (
                    <MenuItem key={member.id} value={member.id}>
                      {member.email}
                    </MenuItem>
                  ))}
              </TextField>
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
          <Button disabled={isSubmitting} type="submit" variant="contained">
            {t('workspace.grantRole')}
          </Button>
        </Stack>
      </Paper>
    </Stack>
  );
};
