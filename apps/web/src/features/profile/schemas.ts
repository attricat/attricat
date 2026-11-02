import { z } from 'zod';
import i18n from 'i18next';
import { tokenLabelMaxLength } from './constants';
import { permissionCodesSchema, uuid } from '../workspace/schemas';

export const tokenSchema = z.object({
  id: uuid,
  label: z.string(),
  permissions: z.array(z.string()),
  expires_at: z.string().nullable(),
  revoked_at: z.string().nullable(),
  last_used_at: z.string().nullable(),
  created_at: z.string(),
});
export type PersonalToken = z.infer<typeof tokenSchema>;
export const tokenInputSchema = z.object({
  label: z.string().trim().min(1).max(tokenLabelMaxLength),
  permissions: permissionCodesSchema.min(1, {
    error: () => i18n.t('profile.tokenPermissionsRequired'),
  }),
  expires_at: z.string().datetime({ offset: true }).optional(),
});
