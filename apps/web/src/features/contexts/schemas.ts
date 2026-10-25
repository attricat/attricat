import { z } from 'zod';
import i18n from '../../i18n';
import { contextCodePattern } from './constants';

const contextCodeSchema = z.string().regex(contextCodePattern, {
  // Resolve lazily so the message follows the language active at validation.
  error: () => i18n.t('contexts.invalidCode'),
});
export const attributeContextSchema = z.object({
  id: z.uuid(),
  code: contextCodeSchema,
  data: z.record(z.string(), z.unknown()),
  parent_id: z.uuid().nullable(),
});
export const createAttributeContextSchema = z.object({
  code: contextCodeSchema,
  data: z.record(z.string(), z.unknown()),
  parent_id: z.uuid(),
});

export type AttributeContext = z.infer<typeof attributeContextSchema>;
