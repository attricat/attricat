import { useQuery } from '@tanstack/react-query';
import { Link } from '@tanstack/react-router';
import { Alert, Box, Button } from '@mui/material';
import { SettingsPage } from '../../components/CenteredPage';
import { PageHeader } from '../../components/PageHeader';
import { WorkspaceIcon } from '../../components/systemIcons';
import { useTranslation } from 'react-i18next';
import { currentSession } from '../auth/api';
import { authQueryKeys } from '../auth/queryKeys';
import { WorkspaceInvitationsSection } from './WorkspaceInvitationsSection';
import { WorkspaceMembersSection } from './WorkspaceMembersSection';
import { WorkspaceNavigationSection } from './WorkspaceNavigationSection';
import { WorkspaceRolesSection } from './WorkspaceRolesSection';

export type WorkspaceManagementSection =
  'members' | 'roles' | 'invitations' | 'navigation';

const sections: {
  labelKey: string;
  section: WorkspaceManagementSection;
  to: string;
}[] = [
  {
    labelKey: 'workspace.members',
    section: 'members',
    to: '/manage/workspace/members',
  },
  {
    labelKey: 'workspace.roles',
    section: 'roles',
    to: '/manage/workspace/roles',
  },
  {
    labelKey: 'workspace.invitations',
    section: 'invitations',
    to: '/manage/workspace/invitations',
  },
  {
    labelKey: 'workspace.navigation',
    section: 'navigation',
    to: '/manage/workspace/navigation',
  },
];

export const WorkspaceManagementPage = ({
  section,
}: {
  section: WorkspaceManagementSection;
}) => {
  const { t } = useTranslation();
  const session = useQuery({
    queryKey: authQueryKeys.session(),
    queryFn: currentSession,
  });
  const capabilities = session.data?.capabilities;

  return (
    <SettingsPage>
      <PageHeader
        description={t('workspace.active', {
          workspace: session.data?.login_identifier ?? t('workspace.loading'),
        })}
        icon={WorkspaceIcon}
        title={t('workspace.title')}
        titleVariant="h4"
      />
      <Box sx={{ display: 'flex', flexWrap: 'wrap', gap: 1, mt: 3 }}>
        {sections
          .filter(
            (item) =>
              (item.section === 'roles' && capabilities?.roles_manage) ||
              ((item.section === 'members' || item.section === 'invitations') &&
                capabilities?.members_manage) ||
              (item.section === 'navigation' &&
                capabilities?.workspace_navigation_manage),
          )
          .map((item) => (
            <Button
              component={Link}
              key={item.section}
              to={item.to}
              variant={item.section === section ? 'contained' : 'outlined'}
            >
              {t(item.labelKey)}
            </Button>
          ))}
      </Box>
      {session.isError && (
        <Alert severity="error">{session.error.message}</Alert>
      )}
      {section === 'members' && (
        <WorkspaceMembersSection
          canManage={capabilities?.members_manage === true}
          canGrantRoles={capabilities?.roles_grant === true}
          currentUserId={session.data?.user_id}
          workspaceId={session.data?.workspace_id}
        />
      )}
      {section === 'roles' && (
        <WorkspaceRolesSection
          canManage={capabilities?.roles_manage === true}
        />
      )}
      {section === 'navigation' && (
        <WorkspaceNavigationSection
          canManage={capabilities?.workspace_navigation_manage === true}
        />
      )}
      {section === 'invitations' && (
        <WorkspaceInvitationsSection
          canManage={capabilities?.members_manage === true}
          workspaceId={session.data?.workspace_id}
        />
      )}
    </SettingsPage>
  );
};
