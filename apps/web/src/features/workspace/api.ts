import { z } from 'zod';
import i18n from 'i18next';
import {
  ApiRequestError,
  request,
  requestNoContent,
  jsonRequest,
} from '../../api/request';
import { httpStatus } from '../../api/httpStatus';
import { uuidSegment } from '../../api/pathSegments';
import { scopeTypes } from './constants';
import {
  uuid,
  scopeTypeSchema,
  memberSchema,
  invitationSchema,
  roleSchema,
  permissionSchema,
  grantTargetSchema,
  exploreNavigationEntrySchema,
  exploreNavigationItemSchema,
  grantInputSchema,
  invitationInputSchema,
  roleInputSchema,
  createdUserSchema,
  createUserInputSchema,
  agentSettingsSchema,
  identityProviderSchema,
  roleCodeSchema,
  type GrantTarget,
  type GrantInput,
  type ScopeType,
  type AgentSettings,
} from './schemas';
export type {
  AgentSettings,
  ExploreNavigationEntry,
  ScopeType,
  WorkspaceRole,
} from './schemas';

export const selectedScopeTarget = (
  scopeType: ScopeType,
  scopeTargetId: string,
  workspaceId: string | undefined,
) => (scopeType === scopeTypes.workspace ? (workspaceId ?? '') : scopeTargetId);

export const ensureActiveScopeTarget = (
  input: GrantInput,
  workspaceId: string | undefined,
  targets: readonly GrantTarget[] | undefined,
) => {
  const parsed = grantInputSchema.parse(input);
  if (parsed.scope_type === scopeTypes.workspace) {
    if (!workspaceId || parsed.scope_target_id !== uuid.parse(workspaceId)) {
      throw new Error(i18n.t('errors.workspaceScopeMismatch'));
    }
    return parsed;
  }
  if (!targets?.some((target) => target.id === parsed.scope_target_id)) {
    throw new Error(i18n.t('errors.scopeTargetRequired'));
  }
  return parsed;
};

export const listMembers = () =>
  request('/api/workspace/members', z.array(memberSchema));
export const grantMemberRole = (memberId: string, input: GrantInput) =>
  request(
    `/api/workspace/members/${uuidSegment(memberId)}/grants`,
    z.object({ id: uuid }),
    jsonRequest('POST', grantInputSchema.parse(input)),
  );
export const unlinkMemberIdentity = (memberId: string) =>
  requestNoContent(`/api/workspace/members/${uuidSegment(memberId)}/identity`, {
    method: 'DELETE',
  });
export const revokeMemberRole = (memberId: string, grantId: string) =>
  requestNoContent(
    `/api/workspace/members/${uuidSegment(memberId)}/grants/${uuidSegment(grantId)}`,
    { method: 'DELETE' },
  );
export const transferOwnership = (id: string) =>
  requestNoContent(
    `/api/workspace/members/${uuidSegment(id)}/transfer-ownership`,
    {
      method: 'POST',
    },
  );

export const createWorkspaceUser = (
  input: z.input<typeof createUserInputSchema>,
) =>
  request(
    '/api/workspace/users',
    createdUserSchema,
    jsonRequest('POST', createUserInputSchema.parse(input)),
  );
export const completeOnboarding = (input: {
  invitation_secret: string;
  onboarding_secret: string;
  password: string;
}) =>
  request(
    '/api/onboarding/complete',
    z.object({ membership_id: uuid }),
    jsonRequest('POST', input),
  );

export const listInvitations = () =>
  request('/api/workspace/invitations', z.array(invitationSchema));
export const createInvitation = (
  input: z.input<typeof invitationInputSchema>,
) =>
  request(
    '/api/workspace/invitations',
    invitationSchema,
    jsonRequest('POST', invitationInputSchema.parse(input)),
  );
export const revokeInvitation = (id: string) =>
  requestNoContent(`/api/workspace/invitations/${uuidSegment(id)}`, {
    method: 'DELETE',
  });
export const acceptInvitation = (secret: string) =>
  request(
    '/api/workspace/invitations/accept',
    z.object({ membership_id: uuid }),
    jsonRequest('POST', { secret: z.string().min(1).parse(secret) }),
  );

export const listRoles = () =>
  request('/api/workspace/roles', z.array(roleSchema));
export const listExploreNavigation = () =>
  request('/api/workspace/navigation', z.array(exploreNavigationEntrySchema));
export const listSidebarExploreNavigation = () =>
  request(
    '/api/workspace/navigation/sidebar',
    z.array(exploreNavigationItemSchema),
  );
export const updateExploreNavigation = (
  explore_navigation: z.input<typeof exploreNavigationEntrySchema>[],
) =>
  requestNoContent(
    '/api/workspace/navigation',
    jsonRequest('PUT', {
      explore_navigation: z
        .array(exploreNavigationEntrySchema)
        .parse(explore_navigation),
    }),
  );
export const getAgentSettings = () =>
  request('/api/workspace/agent-settings', agentSettingsSchema);
export const updateAgentSettings = (settings: AgentSettings) =>
  request(
    '/api/workspace/agent-settings',
    agentSettingsSchema,
    jsonRequest('PUT', agentSettingsSchema.parse(settings)),
  );
/** The workspace's identity provider, or `null` without one. */
export const getIdentityProvider = async () => {
  try {
    return await request(
      '/api/workspace/identity-provider',
      identityProviderSchema,
    );
  } catch (error) {
    if (
      error instanceof ApiRequestError &&
      error.status === httpStatus.notFound
    )
      return null;
    throw error;
  }
};
export const putIdentityProvider = (input: {
  issuer: string;
  client_id: string;
  client_secret?: string;
  display_name: string;
}) =>
  request(
    '/api/workspace/identity-provider',
    identityProviderSchema,
    jsonRequest('PUT', input),
  );
export const updateIdentityProviderState = (input: {
  enabled?: boolean;
  password_sign_in?: string;
}) =>
  request(
    '/api/workspace/identity-provider',
    identityProviderSchema,
    jsonRequest('PATCH', input),
  );
export const deleteIdentityProvider = () =>
  requestNoContent('/api/workspace/identity-provider', { method: 'DELETE' });
export const listAssignableRoles = () =>
  request('/api/workspace/assignable-roles', z.array(roleSchema));
export const listPermissions = () =>
  request('/api/workspace/permissions', z.array(permissionSchema));
export const listGrantTargets = (scope: ScopeType) =>
  request(
    `/api/workspace/grant-targets/${scopeTypeSchema.parse(scope)}`,
    z.array(grantTargetSchema),
  );
export const createRole = (input: z.input<typeof roleInputSchema>) =>
  request(
    '/api/workspace/roles',
    z.object({ id: uuid }),
    jsonRequest('POST', roleInputSchema.parse(input)),
  );
export const updateRole = (
  id: string,
  input: z.input<typeof roleInputSchema>,
) =>
  requestNoContent(
    `/api/workspace/roles/${uuidSegment(id)}`,
    jsonRequest('PUT', roleInputSchema.parse(input)),
  );
export const duplicateRole = (id: string, code: string) =>
  request(
    `/api/workspace/roles/${uuidSegment(id)}/duplicate`,
    z.object({ id: uuid }),
    jsonRequest('POST', {
      code: roleCodeSchema.parse(code),
    }),
  );
export const retireRole = (id: string, replacement_role_id?: string) =>
  requestNoContent(
    `/api/workspace/roles/${uuidSegment(id)}/retire`,
    jsonRequest('POST', {
      ...(replacement_role_id
        ? { replacement_role_id: uuid.parse(replacement_role_id) }
        : {}),
    }),
  );
