import { z } from 'zod';
import { apiFetch } from './request';

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
  const response = await apiFetch('/api/auth/discover', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ login_identifier: loginIdentifier }),
  });
  if (!response.ok) throw new Error('Workspace was not found');
  return discoverySchema.parse(await response.json());
};

export const login = async (
  loginIdentifier: string,
  email: string,
  password: string,
) => {
  const response = await apiFetch('/api/auth/login', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      login_identifier: loginIdentifier,
      email,
      password,
    }),
  });
  if (!response.ok) throw new Error('Invalid email or password');
  return sessionSchema.parse(await response.json());
};

export const requestPasswordReset = async (email: string) => {
  const response = await apiFetch('/api/auth/password-reset', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ email }),
  });
  if (!response.ok) throw new Error('Unable to request a password reset');
};

export const confirmPasswordReset = async (token: string, password: string) => {
  const response = await apiFetch('/api/auth/password-reset/confirm', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ token, password }),
  });
  if (!response.ok)
    throw new Error('This password reset link is invalid or expired');
};

export const currentSession = async () => {
  const response = await apiFetch('/api/auth/session');
  if (response.status === 401) return null;
  if (!response.ok) {
    const statusText = response.statusText ? ` ${response.statusText}` : '';
    throw new Error(
      `Unable to check the current session (HTTP ${response.status}${statusText})`,
    );
  }
  return sessionSchema.parse(await response.json());
};

export const logout = async () => {
  const response = await apiFetch('/api/auth/logout', { method: 'POST' });
  if (!response.ok) {
    const statusText = response.statusText ? ` ${response.statusText}` : '';
    throw new Error(
      `Unable to sign out (HTTP ${response.status}${statusText})`,
    );
  }
};
