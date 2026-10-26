import { useForm } from '@tanstack/react-form';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import {
  Alert,
  Button,
  List,
  ListItem,
  ListItemAvatar,
  ListItemText,
  MenuItem,
  Paper,
  Stack,
  TextField,
  Typography,
} from '@mui/material';
import { UserAvatar } from '../../components/UserAvatar';
import { UserProfileLink } from '../../components/UserProfileLink';
import { userAvatarSizes } from '../../components/userAvatars';
import { invalidateAuthorization } from '../auth/invalidateAuthorization';
import { principalQueryKeys } from '../principals/queryKeys';
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
import { activeMemberState, ownerRoleCode, scopeTypes } from './constants';
import { workspaceQueryKeys } from './queryKeys';
import { ScopeFields } from './ScopeFields';
import { ApiErrorAlert } from '../../components/ApiErrorAlert';
import { useExclusiveMutation } from '../../components/useExclusiveMutation';

export const WorkspaceMembersSection = ({
  canManage,
  canGrantRoles = false,
  currentUserId,
  workspaceId,
}: {
  canManage: boolean;
  canGrantRoles?: boolean;
  currentUserId?: string;
  workspaceId?: string;
}) => {
  const { i18n, t } = useTranslation();
  const roleFormatter = new Intl.ListFormat(i18n.language, {
    style: 'narrow',
    type: 'unit',
  });
  const client = useQueryClient();
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
  const refresh = () => {
    void invalidateAuthorization(client);
    // Assignee pickers list members, so membership changes refresh them too.
    void client.invalidateQueries({ queryKey: principalQueryKeys.directory() });
    return client.invalidateQueries({ queryKey: workspaceQueryKeys.members() });
  };
  const form = useForm({
    defaultValues: {
      member_id: '',
      role_id: '',
      scope_type: scopeTypes.workspace as ScopeType,
      scope_target_id: workspaceId ?? '',
    },
    onSubmit: ({ value }) => grantRole.submit(value),
  });
  const grantRole = useExclusiveMutation({
    mutationFn: (value: typeof form.state.values) => {
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
        client.getQueryData(workspaceQueryKeys.grantTargets(value.scope_type)),
      );
      return grantMemberRole(value.member_id, input);
    },
    onSuccess: () => {
      void refresh();
      form.reset();
    },
  });
  const revoke = useMutation({
    mutationFn: ({
      memberId,
      grantId,
    }: {
      memberId: string;
      grantId: string;
    }) => revokeMemberRole(memberId, grantId),
    onSuccess: refresh,
  });
  const transfer = useMutation({
    mutationFn: transferOwnership,
    onSuccess: refresh,
  });
  const isOwner = members.data?.some(
    (member) =>
      member.user_id === currentUserId &&
      member.grants.some(
        (grant) =>
          grant.role_code === ownerRoleCode &&
          grant.scope_type === scopeTypes.workspace &&
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
      <ApiErrorAlert error={grantRole.error} />
      <ApiErrorAlert error={revoke.error} />
      <ApiErrorAlert error={transfer.error} />
      <ApiErrorAlert error={members.error} />
      <ApiErrorAlert error={roles.error} />
      <Paper>
        <List>
          {members.data?.map((member) => (
            <ListItem divider key={member.id}>
              <ListItemAvatar>
                <UserAvatar
                  avatarFileId={member.avatar_file_id}
                  name={member.display_name ?? member.email}
                  size={userAvatarSizes.list}
                />
              </ListItemAvatar>
              <ListItemText
                primary={
                  <UserProfileLink userId={member.user_id}>
                    {member.display_name ?? member.email}
                  </UserProfileLink>
                }
                secondary={t(
                  member.grants.length
                    ? 'workspace.memberSummaryWithRoles'
                    : 'workspace.memberSummary',
                  {
                    email: member.email,
                    state: t(`workspace.memberStates.${member.state}`, {
                      defaultValue: member.state,
                    }),
                    roles: roleFormatter.format(
                      member.grants.map((grant) => grant.role_code),
                    ),
                  },
                )}
              />
              {canGrantRoles &&
                member.grants.map((grant) => (
                  <Button
                    key={grant.id}
                    disabled={revoke.isPending}
                    onClick={() =>
                      revoke.mutate({ memberId: member.id, grantId: grant.id })
                    }
                  >
                    {t('workspace.revoke', { role: grant.role_code })}
                  </Button>
                ))}
              <Stack direction="row" sx={{ gap: 1 }}>
                {isOwner && (
                  <Button
                    disabled={transfer.isPending}
                    onClick={() => transfer.mutate(member.id)}
                  >
                    {t('workspace.transferOwnership')}
                  </Button>
                )}
              </Stack>
            </ListItem>
          ))}
        </List>
      </Paper>
      {canGrantRoles && (
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
                  required
                  label={t('workspace.member')}
                  onChange={(event) => field.handleChange(event.target.value)}
                  select
                  value={field.state.value}
                >
                  {members.data
                    ?.filter((member) => member.state === activeMemberState)
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
                  required
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
            <Button
              disabled={grantRole.isPending}
              type="submit"
              variant="contained"
            >
              {t('workspace.grantRole')}
            </Button>
          </Stack>
        </Paper>
      )}
    </Stack>
  );
};
