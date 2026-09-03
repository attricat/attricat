import { z } from 'zod';
import { apiFetch } from '../auth/request';
import {
  blueprintSchema,
  blueprintWithAttributesSchema,
  type Blueprint,
  type BlueprintWithAttributes,
} from './schemas';

export type { Attribute, Blueprint, BlueprintWithAttributes } from './schemas';

const request = async <T>(
  path: string,
  schema: z.ZodType<T>,
  init?: RequestInit,
): Promise<T> => {
  const response = init ? await apiFetch(path, init) : await apiFetch(path);
  if (!response.ok) {
    const body = await response.json().catch(() => undefined);
    const error = z
      .object({ error: z.object({ code: z.string(), message: z.string() }) })
      .safeParse(body);
    throw new Error(
      error.success
        ? `${error.data.error.code}: ${error.data.error.message}`
        : `Request failed (${response.status})`,
    );
  }
  const result = schema.safeParse(await response.json());
  if (!result.success)
    throw new Error(`Invalid API response: ${z.prettifyError(result.error)}`);
  return result.data;
};

export const listBlueprints = (): Promise<Blueprint[]> =>
  request('/api/blueprints/catalogue', z.array(blueprintSchema));

export const listBlueprintRevisions = (id: string): Promise<Blueprint[]> =>
  request(
    `/api/blueprints/${encodeURIComponent(z.uuid().parse(id))}/versions`,
    z.array(blueprintSchema),
  );

export const getBlueprintRevision = (
  id: string,
  version: number,
): Promise<BlueprintWithAttributes> =>
  request(
    `/api/blueprints/${encodeURIComponent(z.uuid().parse(id))}/versions/${z.number().int().positive().parse(version)}`,
    blueprintWithAttributesSchema,
  );

export const createBlueprint = (
  definition: string,
): Promise<BlueprintWithAttributes> =>
  request('/api/blueprints', blueprintWithAttributesSchema, {
    body: JSON.stringify({ definition }),
    headers: { 'Content-Type': 'application/json' },
    method: 'POST',
  });

export const createBlueprintRevision = (
  id: string,
  definition: string,
): Promise<BlueprintWithAttributes> =>
  request(
    `/api/blueprints/${encodeURIComponent(z.uuid().parse(id))}/versions`,
    blueprintWithAttributesSchema,
    {
      body: JSON.stringify({ definition }),
      headers: { 'Content-Type': 'application/json' },
      method: 'POST',
    },
  );

export const publishBlueprintRevision = (
  id: string,
  version: number,
): Promise<BlueprintWithAttributes> =>
  request(
    `/api/blueprints/${encodeURIComponent(z.uuid().parse(id))}/versions/${z.number().int().positive().parse(version)}/publish`,
    blueprintWithAttributesSchema,
    { method: 'POST' },
  );
