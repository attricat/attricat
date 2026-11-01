import { useQuery } from '@tanstack/react-query';
import { Navigate } from '@tanstack/react-router';
import { Box } from '@mui/material';
import { SettingsPage } from '../../components/CenteredPage';
import { PageHeader } from '../../components/PageHeader';
import { WorkspaceIcon } from '../../components/systemIcons';
import { useTranslation } from 'react-i18next';
import { WorkspaceAgentSection } from './WorkspaceAgentSection';
import { WorkspaceInvitationsSection } from './WorkspaceInvitationsSection';
import { WorkspaceMembersSection } from './WorkspaceMembersSection';
import { WorkspaceNavigationSection } from './WorkspaceNavigationSection';
import { WorkspaceRolesSection } from './WorkspaceRolesSection';
import { WorkspaceSignInSection } from './WorkspaceSignInSection';
import { WorkspaceTagColorsSection } from './WorkspaceTagColorsSection';
import { WorkspaceTeamsSection } from './WorkspaceTeamsSection';
import { sessionOptions } from '../auth/queryOptions';
import { ApiErrorAlert } from '../../components/ApiErrorAlert';
import { RouterButton } from '../../components/RouterLink';
import { capabilities as capabilityCodes } from '../auth/capabilities';
import type { Capability } from '../auth/useCan';

type WorkspaceManagementSection =
  | 'members'
  | 'teams'
  | 'roles'
  | 'invitations'
  | 'navigation'
  | 'tag-colors'
  | 'agent'
  | 'sign-in';

const sections = [
  {
    labelKey: 'workspace.members',
    section: 'members',
    to: '/manage/workspace/members',
    capability: capabilityCodes.membersManage,
  },
  {
    labelKey: 'workspace.teams',
    section: 'teams',
    to: '/manage/workspace/teams',
    capability: capabilityCodes.membersManage,
  },
  {
    labelKey: 'workspace.roles',
    section: 'roles',
    to: '/manage/workspace/roles',
    capability: capabilityCodes.rolesManage,
  },
  {
    labelKey: 'workspace.invitations',
    section: 'invitations',
    to: '/manage/workspace/invitations',
    capability: capabilityCodes.membersManage,
  },
  {
    labelKey: 'workspace.navigation',
    section: 'navigation',
    to: '/manage/workspace/navigation',
    capability: capabilityCodes.workspaceNavigationManage,
  },
  {
    labelKey: 'workspace.tagColors',
    section: 'tag-colors',
    to: '/manage/workspace/tag-colors',
    capability: capabilityCodes.tagColorsManage,
  },
  {
    labelKey: 'workspace.agent',
    section: 'agent',
    to: '/manage/workspace/agent',
    capability: capabilityCodes.agentsManage,
  },
  {
    labelKey: 'workspace.signIn',
    section: 'sign-in',
    to: '/manage/workspace/sign-in',
    capability: capabilityCodes.workspaceManage,
  },
] as const satisfies readonly {
  labelKey: string;
  section: WorkspaceManagementSection;
  to: string;
  capability: Capability;
}[];

export const WorkspaceManagementPage = ({
  section: requestedSection,
}: {
  /** The section to show; the first one the caller may manage when absent. */
  section?: WorkspaceManagementSection;
}) => {
  const { t } = useTranslation();
  const session = useQuery(sessionOptions());
  const capabilities = session.data?.capabilities;
  const can = (capability: Capability) => capabilities?.[capability] === true;
  const allowedSections = sections.filter((item) => can(item.capability));
  const fallback = allowedSections[0] ?? sections[0];
  if (!requestedSection && session.data)
    return <Navigate replace to={fallback.to} />;
  const section = requestedSection ?? fallback.section;

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
        {allowedSections.map((item) => (
          <RouterButton
            key={item.section}
            to={item.to}
            variant={item.section === section ? 'contained' : 'outlined'}
          >
            {t(item.labelKey)}
          </RouterButton>
        ))}
      </Box>
      <ApiErrorAlert error={session.error} />
      {/* Capabilities are unknown until the session loads, so sections would
          otherwise flash their refusal. */}
      {!session.isPending && (
        <>
          {section === 'members' && (
            <WorkspaceMembersSection
              canManage={can(capabilityCodes.membersManage)}
              canGrantRoles={can(capabilityCodes.rolesGrant)}
              currentUserId={session.data?.user_id}
              workspaceId={session.data?.workspace_id}
            />
          )}
          {section === 'teams' && (
            <WorkspaceTeamsSection
              canManage={can(capabilityCodes.membersManage)}
            />
          )}
          {section === 'roles' && (
            <WorkspaceRolesSection
              canManage={can(capabilityCodes.rolesManage)}
            />
          )}
          {section === 'navigation' && (
            <WorkspaceNavigationSection
              canManage={can(capabilityCodes.workspaceNavigationManage)}
            />
          )}
          {section === 'tag-colors' && (
            <WorkspaceTagColorsSection
              canManage={can(capabilityCodes.tagColorsManage)}
            />
          )}
          {section === 'agent' && (
            <WorkspaceAgentSection
              canManage={can(capabilityCodes.agentsManage)}
            />
          )}
          {section === 'sign-in' && (
            <WorkspaceSignInSection
              canManage={can(capabilityCodes.workspaceManage)}
            />
          )}
          {section === 'invitations' && (
            <WorkspaceInvitationsSection
              canManage={can(capabilityCodes.membersManage)}
              workspaceId={session.data?.workspace_id}
            />
          )}
        </>
      )}
    </SettingsPage>
  );
};
