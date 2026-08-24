import { z } from 'zod';
import { apiFetch } from '../auth/request';
import {
  blueprintSchema,
  blueprintWithAttributesSchema,
  type Blueprint,
  type BlueprintWithAttributes,
} from './schemas';

export type { Attribute, Blueprint, BlueprintWithAttributes } from './schemas';

const request = async <T>(path: string, schema: z.ZodType<T>): Promise<T> => {
  const response = await apiFetch(path);
  if (!response.ok) throw new Error(`Request failed (${response.status})`);
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
