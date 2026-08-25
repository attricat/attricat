import { z } from 'zod';
import { apiFetch } from '../auth/request';

const uuid = z.uuid();
const tokenSchema = z.object({
  id: uuid,
  label: z.string(),
  permissions: z.array(z.string()),
  expires_at: z.string().nullable(),
  revoked_at: z.string().nullable(),
  last_used_at: z.string().nullable(),
  created_at: z.string(),
});
const permissionSchema = z.object({
  code: z.string(),
  description: z.string(),
});
const apiErrorSchema = z.object({ error: z.object({ message: z.string() }) });
const permissionCodesSchema = z
  .array(z.string())
  .refine(
    (permissions) => new Set(permissions).size === permissions.length,
    'Permissions must be unique',
  )
  .min(1);
const tokenInputSchema = z.object({
  label: z.string().trim().min(1).max(120),
  permissions: permissionCodesSchema,
  expires_at: z.string().datetime({ offset: true }).optional(),
});

const request = async <T>(
  path: string,
  schema: z.ZodType<T>,
  init?: RequestInit,
) => {
  const response = await apiFetch(path, init);
  if (!response.ok) {
    const parsed = apiErrorSchema.safeParse(
      await response.json().catch(() => null),
    );
    throw new Error(
      parsed.success
        ? parsed.data.error.message
        : `Request failed (${response.status})`,
    );
  }
  return schema.parse(await response.json());
};

const json = (method: string, value: unknown): RequestInit => ({
  method,
  headers: { 'Content-Type': 'application/json' },
  body: JSON.stringify(value),
});

export const listTokens = () =>
  request('/api/personal-access-tokens', z.array(tokenSchema));
export const listTokenPermissions = () =>
  request('/api/workspace/token-permissions', z.array(permissionSchema));
export const createToken = (input: z.input<typeof tokenInputSchema>) => {
  const parsed = tokenInputSchema.parse(input);
  if (parsed.expires_at && new Date(parsed.expires_at) <= new Date()) {
    throw new Error('Token expiry must be in the future.');
  }
  return request(
    '/api/personal-access-tokens',
    tokenSchema.extend({ secret: z.string().startsWith('cat_pat_') }),
    json('POST', parsed),
  );
};
export const revokeToken = async (id: string) => {
  const response = await apiFetch(`/api/personal-access-tokens/${uuid.parse(id)}`, {
    method: 'DELETE',
  });
  if (!response.ok) throw new Error(`Request failed (${response.status})`);
};
