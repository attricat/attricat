import { z } from 'zod';
import i18n from 'i18next';
import { request, requestNoContent, jsonRequest } from '../../api/request';
import { uuidSegment } from '../../api/pathSegments';
import { tokenSchema, permissionSchema, tokenInputSchema } from './schemas';
export type { PersonalToken } from './schemas';

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
    jsonRequest('POST', parsed),
  );
};
export const revokeToken = (id: string) =>
  requestNoContent(`/api/personal-access-tokens/${uuidSegment(id)}`, {
    method: 'DELETE',
  });
