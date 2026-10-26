import { z } from 'zod';
import { request, jsonRequest } from '../../api/request';
import type { RemovalDisposition } from './constants';
import {
  blueprintRecordPublicationSummarySchema,
  blueprintMigrationBatchSchema,
  blueprintMigrationBatchStatusSchema,
  blueprintMigrationImpactSchema,
  blueprintSchema,
  blueprintWithAttributesSchema,
  type Blueprint,
  type BlueprintWithAttributes,
} from './schemas';
import { uuidSegment, versionSegment } from '../../api/pathSegments';

export type { Blueprint, BlueprintWithAttributes } from './schemas';

export const listBlueprints = (): Promise<Blueprint[]> =>
  request('/api/blueprints/catalogue', z.array(blueprintSchema));

export const listBlueprintRevisions = (
  id: string,
  signal?: AbortSignal,
): Promise<Blueprint[]> =>
  request(
    `/api/blueprints/${uuidSegment(id)}/versions`,
    z.array(blueprintSchema),
    { signal },
  );

export const getBlueprintRevision = (
  id: string,
  version: number,
): Promise<BlueprintWithAttributes> =>
  request(
    `/api/blueprints/${uuidSegment(id)}/versions/${versionSegment(version)}`,
    blueprintWithAttributesSchema,
  );

export const createBlueprint = (
  definition: string,
): Promise<BlueprintWithAttributes> =>
  request(
    '/api/blueprints',
    blueprintWithAttributesSchema,
    jsonRequest('POST', { definition }),
  );

export const createBlueprintRevision = (
  id: string,
  definition: string,
): Promise<BlueprintWithAttributes> =>
  request(
    `/api/blueprints/${uuidSegment(id)}/versions`,
    blueprintWithAttributesSchema,
    jsonRequest('POST', { definition }),
  );

/**
 * Runs every check that saving would, without storing anything. With an id it
 * checks the next revision of that blueprint.
 */
export const validateBlueprint = (
  definition: string,
  id?: string,
  signal?: AbortSignal,
): Promise<BlueprintWithAttributes> =>
  request(
    id === undefined
      ? '/api/blueprints/validate'
      : `/api/blueprints/${uuidSegment(id)}/versions/validate`,
    blueprintWithAttributesSchema,
    {
      ...jsonRequest('POST', { definition }),
      signal,
    },
  );

export const listBlueprintMigrationBatches = (id: string) =>
  request(
    `/api/blueprints/${uuidSegment(id)}/migration-batches`,
    z.array(blueprintMigrationBatchStatusSchema),
  );

export const getSafeBlueprintMigrationImpact = (id: string, version: number) =>
  request(
    `/api/blueprints/${uuidSegment(id)}/versions/${versionSegment(version)}/safe-migration-impact`,
    blueprintMigrationImpactSchema,
  );

export const startSafeBlueprintMigrationBatch = (
  id: string,
  version: number,
  removalDisposition?: RemovalDisposition,
) =>
  request(
    `/api/blueprints/${uuidSegment(id)}/versions/${versionSegment(version)}/safe-migration-batches`,
    blueprintMigrationBatchSchema,
    removalDisposition
      ? jsonRequest('POST', { removal_disposition: removalDisposition })
      : { method: 'POST' },
  );

export const publishBlueprintRecords = (
  id: string,
  version: number,
  contextId: string,
) =>
  request(
    `/api/blueprints/${uuidSegment(id)}/versions/${versionSegment(version)}/record-publications`,
    blueprintRecordPublicationSummarySchema,
    jsonRequest('POST', { context_id: z.uuid().parse(contextId) }),
  );

export const publishBlueprintRecordsAllChannels = (
  id: string,
  version: number,
) =>
  request(
    `/api/blueprints/${uuidSegment(id)}/versions/${versionSegment(version)}/record-publications/publish-all`,
    blueprintRecordPublicationSummarySchema,
    { method: 'POST' },
  );

export const publishBlueprintRevision = (
  id: string,
  version: number,
): Promise<BlueprintWithAttributes> =>
  request(
    `/api/blueprints/${uuidSegment(id)}/versions/${versionSegment(version)}/publish`,
    blueprintWithAttributesSchema,
    { method: 'POST' },
  );
