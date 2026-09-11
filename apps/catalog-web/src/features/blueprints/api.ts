import { z } from 'zod';
import { request } from '../../api/request';
import {
  blueprintSchema,
  blueprintWithAttributesSchema,
  type Blueprint,
  type BlueprintWithAttributes,
} from './schemas';

export type { Attribute, Blueprint, BlueprintWithAttributes } from './schemas';

export const listBlueprints = (): Promise<Blueprint[]> =>
  request('/api/blueprints/catalogue', z.array(blueprintSchema));

export const listBlueprintRevisions = (
  id: string,
  signal?: AbortSignal,
): Promise<Blueprint[]> =>
  request(
    `/api/blueprints/${encodeURIComponent(z.uuid().parse(id))}/versions`,
    z.array(blueprintSchema),
    signal === undefined ? undefined : { signal },
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
