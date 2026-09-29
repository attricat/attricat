import { useQuery, useQueryClient } from '@tanstack/react-query';
import { Alert, Stack } from '@mui/material';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { listAssignableRoles, listInvitations } from './api';
import { CreateUserInviteForm } from './CreateUserInviteForm';
import { InvitationList } from './InvitationList';
import { InviteExistingUserForm } from './InviteExistingUserForm';
import { workspaceQueryKeys } from './queryKeys';

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
      <InvitationList
        invitations={invitations.data}
        onChanged={refresh}
        onError={setError}
      />
      <CreateUserInviteForm
        onCreated={refresh}
        onError={setError}
        roles={roles.data}
        workspaceId={workspaceId}
      />
      <InviteExistingUserForm
        onCreated={refresh}
        onError={setError}
        roles={roles.data}
        workspaceId={workspaceId}
      />
    </Stack>
  );
};
