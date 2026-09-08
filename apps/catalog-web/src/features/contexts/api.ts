import { z } from 'zod';
import { apiFetch } from '../auth/request';

const contextCodeSchema = z
  .string()
  .regex(
    /^[A-Za-z0-9_-]+$/,
    'Use only letters, numbers, hyphens, and underscores',
  );
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

const request = async <T>(
  path: string,
  schema: z.ZodType<T>,
  init?: RequestInit,
): Promise<T> => {
  const response = await apiFetch(path, init);
  if (!response.ok) throw new Error(`Request failed (${response.status})`);

  const result = schema.safeParse(await response.json());
  if (!result.success)
    throw new Error(`Invalid API response: ${z.prettifyError(result.error)}`);
  return result.data;
};

export const listContexts = (): Promise<AttributeContext[]> =>
  request('/api/contexts', z.array(attributeContextSchema));

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

export const deleteContext = async (id: string): Promise<void> => {
  const response = await apiFetch(
    `/api/contexts/id/${encodeURIComponent(z.uuid().parse(id))}`,
    { method: 'DELETE' },
  );
  if (!response.ok) throw new Error(`Request failed (${response.status})`);
};
