import { z } from 'zod';
import { request, requestNoContent } from '../../api/request';
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
const createAttributeContextSchema = z.object({
  code: contextCodeSchema,
  data: z.record(z.string(), z.unknown()),
  parent_id: z.uuid(),
});

export type AttributeContext = z.infer<typeof attributeContextSchema>;

export const listContexts = (
  signal?: AbortSignal,
): Promise<AttributeContext[]> =>
  request(
    '/api/contexts',
    z.array(attributeContextSchema),
    signal === undefined ? undefined : { signal },
  );

export const createContext = (
  code: string,
  data: Record<string, unknown>,
  parentId: string,
): Promise<AttributeContext> => {
  const payload = createAttributeContextSchema.parse({
    code,
    data,
    parent_id: parentId,
  });
  return request('/api/contexts', attributeContextSchema, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(payload),
  });
};

export const updateContext = (
  id: string,
  data: Record<string, unknown>,
  parentId: string,
): Promise<AttributeContext> =>
  request(
    `/api/contexts/id/${encodeURIComponent(z.uuid().parse(id))}`,
    attributeContextSchema,
    {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ data, parent_id: z.uuid().parse(parentId) }),
    },
  );

export const deleteContext = async (id: string): Promise<void> =>
  requestNoContent(
    `/api/contexts/id/${encodeURIComponent(z.uuid().parse(id))}`,
    { method: 'DELETE' },
  );
