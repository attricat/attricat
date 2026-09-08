import { z } from 'zod';
import { request, requestNoContent } from '../../api/request';

const uuid = z.uuid();
const scopeTypeSchema = z.enum([
  'workspace',
  'blueprint_family',
  'entity',
  'context_subtree',
]);
const grantSchema = z.object({
  id: uuid,
  role_id: uuid,
  role_code: z.string(),
  scope_type: scopeTypeSchema,
  scope_target_id: uuid,
});
const memberSchema = z.object({
  id: uuid,
  user_id: uuid,
  email: z.string().email(),
  display_name: z.string().nullable(),
  state: z.enum(['active', 'inactive']),
  created_at: z.string(),
  updated_at: z.string(),
  grants: z.array(grantSchema),
});
const invitationSchema = z.object({
  id: uuid,
  invitee_email: z.string().email(),
  inviter_user_id: uuid,
  inviter_email: z.string().email(),
  role_id: uuid,
  role_code: z.string(),
  scope_type: scopeTypeSchema,
  scope_target_id: uuid,
  expires_at: z.string(),
  accepted_at: z.string().nullable(),
  accepted_by_user_id: uuid.nullable(),
  revoked_at: z.string().nullable(),
  created_at: z.string(),
});
const roleSchema = z.object({
  id: uuid,
  code: z.string(),
  is_system: z.boolean(),
  permissions: z.array(z.string()),
  created_at: z.string(),
});
const permissionSchema = z.object({
  code: z.string(),
  description: z.string(),
});
const grantTargetSchema = z.object({ id: uuid, label: z.string() });
const exploreNavigationEntrySchema = z.object({
  blueprint_code: z.string().min(1),
  visible_to_role_codes: z.array(z.string()).default([]),
});
const exploreNavigationItemSchema = exploreNavigationEntrySchema
  .pick({
    blueprint_code: true,
  })
  .extend({ blueprint_name: z.string() });

const permissionCodesSchema = z
  .array(z.string())
  .refine(
    (permissions) => new Set(permissions).size === permissions.length,
    'Permissions must be unique',
  );
const grantInputSchema = z.object({
  role_id: uuid,
  scope_type: scopeTypeSchema,
  scope_target_id: uuid,
});
const invitationInputSchema = grantInputSchema.extend({
  email: z.string().email(),
  expires_at: z.string().datetime({ offset: true }),
});
const roleInputSchema = z.object({
  code: z.string().regex(/^[a-z][a-z0-9_-]*$/),
  permissions: permissionCodesSchema,
});

export type WorkspaceMember = z.infer<typeof memberSchema>;
export type WorkspaceRole = z.infer<typeof roleSchema>;
export type Permission = z.infer<typeof permissionSchema>;
export type GrantTarget = z.infer<typeof grantTargetSchema>;
export type ScopeType = z.infer<typeof scopeTypeSchema>;
export type GrantInput = z.infer<typeof grantInputSchema>;
export type ExploreNavigationEntry = z.infer<
  typeof exploreNavigationEntrySchema
>;
export type ExploreNavigationItem = z.infer<typeof exploreNavigationItemSchema>;

export const selectedScopeTarget = (
  scopeType: ScopeType,
  scopeTargetId: string,
  workspaceId: string | undefined,
) => (scopeType === 'workspace' ? (workspaceId ?? '') : scopeTargetId);

export const ensureActiveScopeTarget = (
  input: GrantInput,
  workspaceId: string | undefined,
  targets: readonly GrantTarget[] | undefined,
) => {
  const parsed = grantInputSchema.parse(input);
  if (parsed.scope_type === 'workspace') {
    if (!workspaceId || parsed.scope_target_id !== uuid.parse(workspaceId)) {
      throw new Error('Workspace scope must target the active workspace.');
    }
    return parsed;
  }
  if (!targets?.some((target) => target.id === parsed.scope_target_id)) {
    throw new Error(
      'Choose a target from the selected scope before submitting.',
    );
  }
  return parsed;
};

const json = (method: string, value: unknown): RequestInit => ({
  method,
  headers: { 'Content-Type': 'application/json' },
  body: JSON.stringify(value),
});

export const listMembers = () =>
  request('/api/workspace/members', z.array(memberSchema));
export const setMemberState = (id: string, state: 'active' | 'inactive') =>
  requestNoContent(
    `/api/workspace/members/${uuid.parse(id)}`,
    json('PUT', { state }),
  );
export const grantMemberRole = (memberId: string, input: GrantInput) =>
  request(
    `/api/workspace/members/${uuid.parse(memberId)}/grants`,
    z.object({ id: uuid }),
    json('POST', grantInputSchema.parse(input)),
  );
export const revokeMemberRole = (memberId: string, grantId: string) =>
  requestNoContent(
    `/api/workspace/members/${uuid.parse(memberId)}/grants/${uuid.parse(grantId)}`,
    { method: 'DELETE' },
  );
export const transferOwnership = (id: string) =>
  requestNoContent(
    `/api/workspace/members/${uuid.parse(id)}/transfer-ownership`,
    {
      method: 'POST',
    },
  );

const createdUserSchema = z.object({
  user_id: uuid,
  invitation: invitationSchema.nullable(),
});
const createUserInputSchema = invitationInputSchema.extend({
  display_name: z.string().min(1).optional(),
});

export const createWorkspaceUser = (
  input: z.input<typeof createUserInputSchema>,
) =>
  request(
    '/api/workspace/users',
    createdUserSchema,
    json('POST', createUserInputSchema.parse(input)),
  );
export const completeOnboarding = (input: {
  invitation_secret: string;
  onboarding_secret: string;
  password: string;
}) =>
  request(
    '/api/onboarding/complete',
    z.object({ membership_id: uuid }),
    json('POST', input),
  );

export const listInvitations = () =>
  request('/api/workspace/invitations', z.array(invitationSchema));
export const createInvitation = (
  input: z.input<typeof invitationInputSchema>,
) =>
  request(
    '/api/workspace/invitations',
    invitationSchema,
    json('POST', invitationInputSchema.parse(input)),
  );
export const revokeInvitation = (id: string) =>
  requestNoContent(`/api/workspace/invitations/${uuid.parse(id)}`, {
    method: 'DELETE',
  });
export const acceptInvitation = (secret: string) =>
  request(
    '/api/workspace/invitations/accept',
    z.object({ membership_id: uuid }),
    json('POST', { secret: z.string().min(1).parse(secret) }),
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
    json('PUT', {
      explore_navigation: z
        .array(exploreNavigationEntrySchema)
        .parse(explore_navigation),
    }),
  );
export const listAssignableRoles = () =>
  request('/api/workspace/assignable-roles', z.array(roleSchema));
export const listPermissions = () =>
  request('/api/workspace/permissions', z.array(permissionSchema));
export const listTokenPermissions = () =>
  request('/api/workspace/token-permissions', z.array(permissionSchema));
export const listGrantTargets = (scope: ScopeType) =>
  request(
    `/api/workspace/grant-targets/${scopeTypeSchema.parse(scope)}`,
    z.array(grantTargetSchema),
  );
export const createRole = (input: z.input<typeof roleInputSchema>) =>
  request(
    '/api/workspace/roles',
    z.object({ id: uuid }),
    json('POST', roleInputSchema.parse(input)),
  );
export const updateRole = (
  id: string,
  input: z.input<typeof roleInputSchema>,
) =>
  requestNoContent(
    `/api/workspace/roles/${uuid.parse(id)}`,
    json('PUT', roleInputSchema.parse(input)),
  );
export const duplicateRole = (id: string, code: string) =>
  request(
    `/api/workspace/roles/${uuid.parse(id)}/duplicate`,
    z.object({ id: uuid }),
    json('POST', {
      code: z
        .string()
        .regex(/^[a-z][a-z0-9_-]*$/)
        .parse(code),
    }),
  );
export const retireRole = (id: string, replacement_role_id?: string) =>
  requestNoContent(
    `/api/workspace/roles/${uuid.parse(id)}/retire`,
    json('POST', {
      ...(replacement_role_id
        ? { replacement_role_id: uuid.parse(replacement_role_id) }
        : {}),
    }),
  );
