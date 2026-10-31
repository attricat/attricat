import { z } from 'zod';
import { request, jsonRequest, requestNoContent } from '../../api/request';
import {
  attributeContextSchema,
  createAttributeContextSchema,
  type AttributeContext,
} from './schemas';
export type { AttributeContext } from './schemas';

export const listContexts = (
  signal?: AbortSignal,
): Promise<AttributeContext[]> =>
  request('/api/contexts', z.array(attributeContextSchema), { signal });

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
  return request(
    '/api/contexts',
    attributeContextSchema,
    jsonRequest('POST', payload),
  );
};

export const deleteContext = (id: string) =>
  requestNoContent(`/api/contexts/id/${encodeURIComponent(id)}`, {
    method: 'DELETE',
  });
