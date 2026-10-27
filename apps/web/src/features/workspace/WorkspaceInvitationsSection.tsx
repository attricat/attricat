import { useQuery, useQueryClient } from '@tanstack/react-query';
import { Alert, Stack } from '@mui/material';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { listInvitations } from './api';
import { InvitationList } from './InvitationList';
import { InvitationForm } from './InvitationForm';
import { principalQueryKeys } from '../principals/queryKeys';
import { workspaceQueryKeys } from './queryKeys';
import { ApiErrorAlert } from '../../components/ApiErrorAlert';
import { assignableRolesOptions } from './queryOptions';

export const WorkspaceInvitationsSection = ({
  canManage,
  workspaceId,
}: {
  canManage: boolean;
  workspaceId?: string;
}) => {
  const { t } = useTranslation();
  const client = useQueryClient();
  const [error, setError] = useState<Error>();
  const invitations = useQuery({
    enabled: canManage,
    queryKey: workspaceQueryKeys.invitations(),
    queryFn: listInvitations,
  });
  const roles = useQuery({
    enabled: canManage,
    ...assignableRolesOptions(),
  });
  const refresh = () => {
    // Joined members become assignable, so refresh the assignee directory.
    void client.invalidateQueries({ queryKey: principalQueryKeys.directory() });
    return client.invalidateQueries({
      queryKey: workspaceQueryKeys.invitations(),
    });
  };
  if (!canManage) {
    return (
      <Alert severity="error">{t('workspace.notAuthorizedInvitations')}</Alert>
    );
  }
  return (
    <Stack spacing={2} sx={{ mt: 3 }}>
      <ApiErrorAlert error={error} />
      <ApiErrorAlert error={invitations.error} />
      <ApiErrorAlert error={roles.error} />
      <InvitationList
        invitations={invitations.data}
        onChanged={refresh}
        onError={setError}
      />
      <InvitationForm
        createUser
        onCreated={refresh}
        onError={setError}
        roles={roles.data}
        workspaceId={workspaceId}
      />
      <InvitationForm
        onCreated={refresh}
        onError={setError}
        roles={roles.data}
        workspaceId={workspaceId}
      />
    </Stack>
  );
};
