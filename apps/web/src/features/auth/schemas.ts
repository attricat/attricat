import { z } from 'zod';
import { FILE_STATUS_VALUES } from '../files/constants';

export const ownAvatarSchema = z.object({
  file_id: z.uuid(),
  status: z.enum(FILE_STATUS_VALUES),
});

export const sessionSchema = z.object({
  user_id: z.uuid(),
  display_name: z.string().nullable(),
  email: z.string().email(),
  /** IANA zone for rendering instants; `null` follows the browser zone. */
  time_zone: z.string().nullable().default(null),
  /** The caller's avatar in this workspace, including one still processing. */
  avatar: ownAvatarSchema.nullable().default(null),
  workspace_id: z.uuid(),
  login_identifier: z.string(),
  capabilities: z
    .object({
      audit_read: z.boolean(),
      data_health_read: z.boolean().default(false),
      members_manage: z.boolean(),
      roles_manage: z.boolean(),
      roles_grant: z.boolean().default(false),
      tokens_manage: z.boolean(),
      workspace_navigation_manage: z.boolean(),
      agents_manage: z.boolean().default(false),
      extensions_read: z.boolean(),
      extensions_manage: z.boolean(),
      workflows_read: z.boolean().default(false),
      workflows_manage: z.boolean().default(false),
      rules_read: z.boolean().default(false),
      rules_manage: z.boolean().default(false),
      records_publish: z.boolean().default(false),
      records_unlock: z.boolean().default(false),
      records_delete: z.boolean().default(false),
      blueprints_write: z.boolean().default(false),
      records_write: z.boolean().default(false),
      records_read: z.boolean().default(false),
      blueprints_publish: z.boolean().default(false),
      contexts_write: z.boolean().default(false),
      agents_run: z.boolean().default(false),
    })
    .optional(),
});

export type Session = z.infer<typeof sessionSchema>;

export type UserPreferences = Pick<Session, 'time_zone'>;

export const discoverySchema = z.object({
  login_identifier: z.string(),
  sign_in_methods: z.array(z.string()),
});

/** Seeded per-role accounts sharing one password; `null` unless configured. */
export const sampleLoginsSchema = z
  .object({
    demo: z.boolean(),
    login_identifier: z.string(),
    password: z.string(),
    /** Ordered from least to most privileged. */
    accounts: z.array(z.object({ role: z.string(), email: z.string() })),
  })
  .nullable();

export type SampleLogins = NonNullable<z.infer<typeof sampleLoginsSchema>>;

export const accessSchema = z.object({ granted: z.array(z.uuid()) });
