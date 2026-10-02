import { z } from 'zod';
import { request, requestNoContent } from '../../api/request';
import {
  ENTITY_CHANGES_PAGE_SIZE,
  ENTITY_PREVIEW_QUERY,
  ENTITY_SEARCH_PAGE_SIZE,
} from './constants';
import {
  blueprintSchema,
  blueprintWithAttributesSchema,
  createEntityRequestSchema,
  entityHierarchySchema,
  entityAuditChangeSchema,
  entityFormResponseSchema,
  entityMigrationPreviewSchema,
  entityPreviewResponseSchema,
  entitySchema,
  entitySearchResponseSchema,
  getBlueprintRequestSchema,
  getBlueprintRevisionRequestSchema,
  incomingRelationshipsPageSchema,
  resolvedEntityPreviewSchema,
  smartFillEntityFormRequestSchema,
  smartFillEntityFormResponseSchema,
  searchEntitiesRequestSchema,
  migrateEntityRequestSchema,
  updateEntityRequestSchema,
  entityPublicationStatusSchema,
  uuidSchema,
} from './schemas';

export { viewBlockTypes } from './schemas';
export type {
  Attribute,
  Blueprint,
  BlueprintWithAttributes,
  ComponentReference,
  Entity,
  EntityAuditChange,
  EntityFormResponse,
  EntityMigrationPreview,
  EntityPublicationStatus,
  FormAttributeValue,
  EntityItem,
  EntitySearchFilter,
  EntitySearchResponse,
  JsonSchema,
  NewAttributeValue,
  RelationshipTargets,
  ResolvedEntityPreview,
  ViewDefinition,
  ViewNode,
} from './schemas';

export { ApiRequestError } from '../../api/request';

export type SearchEntitiesOptions = {
  blueprint: string;
  cursor?: string | null;
  filters?: import('./schemas').EntitySearchFilter[];
  includeTotal?: boolean;
  query?: string;
  relationshipFilters?: {
    field: string;
    selected_target_ids: string[];
  }[];
  signal?: AbortSignal;
  sort?: { field: string; direction: 'asc' | 'desc'; context_code?: string };
  version?: number;
};

export const searchEntities = ({
  blueprint,
  cursor = null,
  filters = [],
  includeTotal = false,
  query = '',
  relationshipFilters = [],
  signal,
  sort,
  version,
}: SearchEntitiesOptions) => {
  const payload = searchEntitiesRequestSchema.parse({
    blueprint: {
      code: blueprint,
      ...(version === undefined ? {} : { version }),
    },
    query,
    filters,
    ...(relationshipFilters.length
      ? { relationship_filters: relationshipFilters }
      : {}),
    sort,
    ...(includeTotal ? { include_total: true } : {}),
    page: { size: ENTITY_SEARCH_PAGE_SIZE, cursor },
  });
  return request('/api/v1/entities/search', entitySearchResponseSchema, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(payload),
    ...(signal === undefined ? {} : { signal }),
  });
};

export const listEntityBlueprints = (signal?: AbortSignal) =>
  request(
    '/api/blueprints',
    z.array(blueprintSchema),
    signal === undefined ? undefined : { signal },
  );
export const getEntityPreview = (id: string, signal?: AbortSignal) =>
  request(
    `/api/entities/${encodeURIComponent(uuidSchema.parse(id))}/preview?relationship_depth=${ENTITY_PREVIEW_QUERY.relationshipDepth}&relationship_limit=${ENTITY_PREVIEW_QUERY.relationshipLimit}`,
    entityPreviewResponseSchema,
    signal === undefined ? undefined : { signal },
  );
export const getResolvedEntityPreview = (id: string, contextId: string) =>
  request(
    `/api/entities/${encodeURIComponent(uuidSchema.parse(id))}/resolved-preview?context_id=${encodeURIComponent(uuidSchema.parse(contextId))}`,
    resolvedEntityPreviewSchema,
  );
export const getEntityHierarchy = (
  id: string,
  contextId: string,
  field: string,
) =>
  request(
    `/api/entities/${encodeURIComponent(uuidSchema.parse(id))}/hierarchy?context_id=${encodeURIComponent(uuidSchema.parse(contextId))}&field=${encodeURIComponent(field)}`,
    entityHierarchySchema,
  );
export const getBlueprintByCode = (
  code: string,
  version?: number,
  signal?: AbortSignal,
) => {
  const input = getBlueprintRequestSchema.parse({ code, version });
  const path = `/api/blueprints/by-code/${encodeURIComponent(input.code)}${input.version === undefined ? '' : `/versions/${input.version}`}`;
  return request(
    path,
    blueprintWithAttributesSchema,
    signal === undefined ? undefined : { signal },
  );
};
export const getBlueprintRevision = (id: string, version: number) => {
  const input = getBlueprintRevisionRequestSchema.parse({ id, version });
  return request(
    `/api/blueprints/${encodeURIComponent(input.id)}/versions/${input.version}`,
    blueprintWithAttributesSchema,
  );
};
export const getCurrentBlueprint = (id: string) =>
  request(
    `/api/blueprints/${encodeURIComponent(uuidSchema.parse(id))}`,
    blueprintWithAttributesSchema,
  );
export const getEntityChanges = (id: string, offset: number) =>
  request(
    `/api/entities/${encodeURIComponent(uuidSchema.parse(id))}/changes?limit=${ENTITY_CHANGES_PAGE_SIZE}&offset=${offset}`,
    z.object({
      items: z.array(entityAuditChangeSchema),
      next_offset: z.number().int().nonnegative().nullable(),
    }),
  );
export const getEntityForm = (id: string, signal?: AbortSignal) => {
  const entityId = uuidSchema.parse(id);
  return request(
    `/api/v1/entities/${encodeURIComponent(entityId)}`,
    entityFormResponseSchema,
    signal === undefined ? undefined : { signal },
  );
};
export const getIncomingRelationships = (
  id: string,
  relationships: { source_blueprint: string; field: string }[],
  pageSize: number,
  cursor: string | null,
) =>
  request(
    `/api/v1/entities/${encodeURIComponent(uuidSchema.parse(id))}/incoming-relationships`,
    incomingRelationshipsPageSchema,
    {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        relationships,
        page: { size: pageSize, cursor },
      }),
    },
  );
export const createEntity = (
  input: z.input<typeof createEntityRequestSchema>,
) => {
  const payload = createEntityRequestSchema.parse(input);
  return request('/api/v1/entities', entitySchema, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(payload),
  });
};
export const smartFillEntityForm = (
  input: z.input<typeof smartFillEntityFormRequestSchema>,
) => {
  const payload = smartFillEntityFormRequestSchema.parse(input);
  return request('/api/agent/smart-fill', smartFillEntityFormResponseSchema, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(payload),
  });
};

export const deleteEntity = (id: string) =>
  requestNoContent(
    `/api/entities/${encodeURIComponent(uuidSchema.parse(id))}`,
    {
      method: 'DELETE',
    },
  );

export const duplicateEntity = (id: string) =>
  request(
    `/api/v1/entities/${encodeURIComponent(uuidSchema.parse(id))}/duplicate`,
    entitySchema,
    { method: 'POST' },
  );

export const updateEntity = (
  id: string,
  input: z.input<typeof updateEntityRequestSchema>,
) => {
  const entityId = uuidSchema.parse(id);
  const payload = updateEntityRequestSchema.parse(input);
  return request(
    `/api/v1/entities/${encodeURIComponent(entityId)}`,
    entitySchema,
    {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(payload),
    },
  );
};

export const getEntityPublications = (id: string, signal?: AbortSignal) =>
  request(
    `/api/v1/entities/${encodeURIComponent(uuidSchema.parse(id))}/publications`,
    z.array(entityPublicationStatusSchema),
    signal === undefined ? undefined : { signal },
  );
export const publishEntity = (id: string, contextId: string) =>
  request(
    `/api/v1/entities/${encodeURIComponent(uuidSchema.parse(id))}/publications`,
    entityPublicationStatusSchema,
    {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ context_id: uuidSchema.parse(contextId) }),
    },
  );
export const publishEntityAllChannels = (id: string) =>
  request(
    `/api/v1/entities/${encodeURIComponent(uuidSchema.parse(id))}/publications/publish-all`,
    z.array(entityPublicationStatusSchema),
    { method: 'POST' },
  );
export const unpublishEntity = (id: string, contextId: string) =>
  requestNoContent(
    `/api/v1/entities/${encodeURIComponent(uuidSchema.parse(id))}/publications/unpublish`,
    {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ context_id: uuidSchema.parse(contextId) }),
    },
  );
export const previewEntityMigration = (id: string) =>
  request(
    `/api/v1/entities/${encodeURIComponent(uuidSchema.parse(id))}/blueprint-migration/preview`,
    entityMigrationPreviewSchema,
    { method: 'POST' },
  );

export const migrateEntity = (
  id: string,
  input: z.input<typeof migrateEntityRequestSchema>,
) => {
  const entityId = uuidSchema.parse(id);
  const { expected_updated_at, ...payload } =
    migrateEntityRequestSchema.parse(input);
  const query = expected_updated_at
    ? `?${new URLSearchParams({ expected_updated_at })}`
    : '';
  return request(
    `/api/v1/entities/${encodeURIComponent(entityId)}/blueprint-migration${query}`,
    entitySchema,
    {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(payload),
    },
  );
};
