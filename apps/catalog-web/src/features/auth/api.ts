import { z } from 'zod';
import { ApiRequestError, request, requestNoContent } from '../../api/request';

const sessionSchema = z.object({
  user_id: z.uuid(),
  display_name: z.string().nullable(),
  email: z.string().email(),
  workspace_id: z.uuid(),
  login_identifier: z.string(),
  capabilities: z
    .object({
      audit_read: z.boolean(),
      members_manage: z.boolean(),
      roles_manage: z.boolean(),
      tokens_manage: z.boolean(),
      workspace_navigation_manage: z.boolean(),
      extensions_read: z.boolean(),
      extensions_manage: z.boolean(),
    })
    .optional(),
});

const discoverySchema = z.object({
  login_identifier: z.string(),
  sign_in_methods: z.array(z.string()),
});

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

export const logout = () =>
  requestNoContent('/api/auth/logout', { method: 'POST' });
