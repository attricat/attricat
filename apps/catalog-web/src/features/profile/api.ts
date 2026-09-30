import { z } from 'zod';
import i18n from 'i18next';
import { request, requestNoContent } from '../../api/request';

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
export type PersonalToken = z.infer<typeof tokenSchema>;
const permissionSchema = z.object({
  code: z.string(),
  description: z.string(),
});
const permissionCodesSchema = z
  .array(z.string())
  .refine((permissions) => new Set(permissions).size === permissions.length, {
    error: () => i18n.t('errors.permissionsUnique'),
  })
  .min(1);
const tokenInputSchema = z.object({
  label: z.string().trim().min(1).max(120),
  permissions: permissionCodesSchema,
  expires_at: z.string().datetime({ offset: true }).optional(),
});

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
    throw new Error(i18n.t('errors.tokenExpiryFuture'));
  }
  return request(
    '/api/personal-access-tokens',
    tokenSchema.extend({ secret: z.string().startsWith('cat_pat_') }),
    json('POST', parsed),
  );
};
export const revokeToken = (id: string) =>
  requestNoContent(`/api/personal-access-tokens/${uuid.parse(id)}`, {
    method: 'DELETE',
  });
