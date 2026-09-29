import { z } from 'zod';
import { request } from '../../api/request';
import type { RemovalDisposition } from './constants';
import {
  blueprintEntityPublicationSummarySchema,
  blueprintMigrationBatchSchema,
  blueprintMigrationBatchStatusSchema,
  blueprintMigrationImpactSchema,
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

export const listBlueprintMigrationBatches = (id: string) =>
  request(
    `/api/blueprints/${encodeURIComponent(z.uuid().parse(id))}/migration-batches`,
    z.array(blueprintMigrationBatchStatusSchema),
  );

export const getSafeBlueprintMigrationImpact = (id: string, version: number) =>
  request(
    `/api/blueprints/${encodeURIComponent(z.uuid().parse(id))}/versions/${z.number().int().positive().parse(version)}/safe-migration-impact`,
    blueprintMigrationImpactSchema,
  );

export const startSafeBlueprintMigrationBatch = (
  id: string,
  version: number,
  removalDisposition?: RemovalDisposition,
) =>
  request(
    `/api/blueprints/${encodeURIComponent(z.uuid().parse(id))}/versions/${z.number().int().positive().parse(version)}/safe-migration-batches`,
    blueprintMigrationBatchSchema,
    removalDisposition
      ? {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ removal_disposition: removalDisposition }),
        }
      : { method: 'POST' },
  );

export const publishBlueprintEntities = (
  id: string,
  version: number,
  contextId: string,
) =>
  request(
    `/api/blueprints/${encodeURIComponent(z.uuid().parse(id))}/versions/${z.number().int().positive().parse(version)}/entity-publications`,
    blueprintEntityPublicationSummarySchema,
    {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ context_id: z.uuid().parse(contextId) }),
    },
  );

export const publishBlueprintEntitiesAllChannels = (
  id: string,
  version: number,
) =>
  request(
    `/api/blueprints/${encodeURIComponent(z.uuid().parse(id))}/versions/${z.number().int().positive().parse(version)}/entity-publications/publish-all`,
    blueprintEntityPublicationSummarySchema,
    { method: 'POST' },
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
