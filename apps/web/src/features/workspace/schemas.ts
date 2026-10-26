import { z } from 'zod';
import i18n from 'i18next';
import {
  bulkEditPolicies,
  maxBulkEditMaxRecords,
  scopeTypes,
} from './constants';

export const uuid = z.uuid();
export const scopeTypeSchema = z.enum(scopeTypes);
const grantSchema = z.object({
  id: uuid,
  role_id: uuid,
  role_code: z.string(),
  scope_type: scopeTypeSchema,
  scope_target_id: uuid,
});
export const memberSchema = z.object({
  id: uuid,
  user_id: uuid,
  email: z.string().email(),
  display_name: z.string().nullable(),
  /** The member's ready avatar in this workspace. */
  avatar_file_id: uuid.nullable().default(null),
  state: z.enum(['active', 'inactive']),
  created_at: z.string(),
  updated_at: z.string(),
  grants: z.array(grantSchema),
});
export const invitationSchema = z.object({
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
export const roleSchema = z.object({
  id: uuid,
  code: z.string(),
  is_system: z.boolean(),
  permissions: z.array(z.string()),
  created_at: z.string(),
});
export const permissionSchema = z.object({
  code: z.string(),
  description: z.string(),
});
export const grantTargetSchema = z.object({ id: uuid, label: z.string() });
export const exploreNavigationEntrySchema = z.object({
  blueprint_code: z.string().min(1),
  visible_to_role_codes: z.array(z.string()).default([]),
});
export const exploreNavigationItemSchema = exploreNavigationEntrySchema
  .pick({
    blueprint_code: true,
  })
  .extend({ blueprint_name: z.string() });

const permissionCodesSchema = z
  .array(z.string())
  .refine((permissions) => new Set(permissions).size === permissions.length, {
    error: () => i18n.t('errors.permissionsUnique'),
  });
export const grantInputSchema = z.object({
  role_id: uuid,
  scope_type: scopeTypeSchema,
  scope_target_id: uuid,
});
export const invitationInputSchema = grantInputSchema.extend({
  email: z.string().email(),
  expires_at: z.string().datetime({ offset: true }),
});
export const roleCodeSchema = z.string().regex(/^[a-z][a-z0-9_-]*$/);
export const roleInputSchema = z.object({
  code: roleCodeSchema,
  permissions: permissionCodesSchema,
});

export type WorkspaceRole = z.infer<typeof roleSchema>;
export type GrantTarget = z.infer<typeof grantTargetSchema>;
export type ScopeType = z.infer<typeof scopeTypeSchema>;
export type GrantInput = z.infer<typeof grantInputSchema>;
export type ExploreNavigationEntry = z.infer<
  typeof exploreNavigationEntrySchema
>;

export const createdUserSchema = z.object({
  user_id: uuid,
  invitation: invitationSchema.nullable(),
});
export const createUserInputSchema = invitationInputSchema.extend({
  display_name: z.string().min(1).optional(),
});

export const agentSettingsSchema = z.object({
  bulk_edits: z.enum(bulkEditPolicies),
  bulk_edit_max_records: z.number().int().min(1).max(maxBulkEditMaxRecords),
});
export type AgentSettings = z.infer<typeof agentSettingsSchema>;
