import { z } from 'zod';
import { apiFetch } from './request';

const sessionSchema = z.object({ user_id: z.uuid() });

export const login = async (email: string, password: string) => {
  const response = await apiFetch('/api/auth/login', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ email, password }),
  });
  if (!response.ok) throw new Error('Invalid email or password');
  return sessionSchema.parse(await response.json());
};

export const currentSession = async () => {
  const response = await apiFetch('/api/auth/session');
  if (!response.ok) return null;
  return sessionSchema.parse(await response.json());
};

export const logout = () => apiFetch('/api/auth/logout', { method: 'POST' });
