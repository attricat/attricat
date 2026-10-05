import { z } from 'zod';
import {
  ApiRequestError,
  request,
  requestNoContent,
  requestUpload,
} from '../../api/request';
import { FILE_STATUS_VALUES } from '../files/constants';

const ownAvatarSchema = z.object({
  file_id: z.uuid(),
  status: z.enum(FILE_STATUS_VALUES),
});

const sessionSchema = z.object({
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
      extensions_read: z.boolean(),
      extensions_manage: z.boolean(),
      workflows_read: z.boolean().default(false),
      workflows_manage: z.boolean().default(false),
      rules_read: z.boolean().default(false),
      rules_manage: z.boolean().default(false),
      entities_publish: z.boolean().default(false),
      entities_delete: z.boolean().default(false),
      blueprints_write: z.boolean().default(false),
    })
    .optional(),
});

export type Session = z.infer<typeof sessionSchema>;

export type UserPreferences = Pick<Session, 'time_zone'>;

const discoverySchema = z.object({
  login_identifier: z.string(),
  sign_in_methods: z.array(z.string()),
});

/** Seeded per-role accounts sharing one password; `null` unless configured. */
const sampleLoginsSchema = z
  .object({
    demo: z.boolean(),
    login_identifier: z.string(),
    password: z.string(),
    /** Ordered from least to most privileged. */
    accounts: z.array(z.object({ role: z.string(), email: z.string() })),
  })
  .nullable();

export type SampleLogins = NonNullable<z.infer<typeof sampleLoginsSchema>>;

export const fetchSampleLogins = async () =>
  request('/api/auth/sample-logins', sampleLoginsSchema);

export const discoverWorkspace = async (loginIdentifier: string) => {
  return request('/api/auth/discover', discoverySchema, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ login_identifier: loginIdentifier }),
  });
};

export const login = async (
  loginIdentifier: string,
  email: string,
  password: string,
) => {
  return request('/api/auth/login', sessionSchema, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      login_identifier: loginIdentifier,
      email,
      password,
    }),
  });
};

export const requestPasswordReset = async (email: string) => {
  return requestNoContent('/api/auth/password-reset', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ email }),
  });
};

export const confirmPasswordReset = async (token: string, password: string) => {
  return requestNoContent('/api/auth/password-reset/confirm', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ token, password }),
  });
};

export const currentSession = async () => {
  try {
    return await request('/api/auth/session', sessionSchema);
  } catch (error) {
    if (error instanceof ApiRequestError && error.status === 401) return null;
    throw error;
  }
};

export const updatePreferences = (preferences: UserPreferences) =>
  request('/api/auth/preferences', sessionSchema, {
    method: 'PATCH',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(preferences),
  });

export const updateDisplayName = (displayName: string) =>
  request('/api/auth/display-name', sessionSchema, {
    method: 'PUT',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ display_name: displayName }),
  });

export const uploadAvatar = (
  file: File,
  onProgress?: (progress: number) => void,
) => {
  const data = new FormData();
  data.append('file', file);
  return requestUpload(
    '/api/auth/avatar',
    data,
    ownAvatarSchema,
    onProgress,
    'PUT',
  );
};

export const removeAvatar = () =>
  requestNoContent('/api/auth/avatar', { method: 'DELETE' });

export const logout = () =>
  requestNoContent('/api/auth/logout', { method: 'POST' });
