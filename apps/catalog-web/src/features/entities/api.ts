import { z } from 'zod';
import { request, requestNoContent } from '../../api/request';
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
  relationshipTreeFacetChildrenResponseSchema,
  resolvedEntityPreviewSchema,
  searchEntitiesRequestSchema,
  migrateEntityRequestSchema,
  updateEntityRequestSchema,
  entityPublicationStatusSchema,
  publicationChannelSchema,
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
  PublicationChannel,
  FormAttributeValue,
  EntityItem,
  EntitySearchFilter,
  EntitySearchResponse,
  IncomingRelationshipsPage,
  JsonSchema,
  NewAttributeValue,
  RelationshipTargets,
  RelationshipTreeFacetChildrenResponse,
  ResolvedEntityPreview,
  ViewDefinition,
  ViewNode,
} from './schemas';

export { ApiRequestError } from '../../api/request';

export const searchEntities = (
  blueprint: string,
  version: number | undefined,
  query: string,
  cursor: string | null = null,
  relationshipTreeFacets?: {
    source_relationship_field: string;
    hierarchy_field?: string;
    context_id: string;
    selected_target_ids: string[];
  }[],
  signal?: AbortSignal,
  sort?: { field: string; direction: 'asc' | 'desc' },
  includeTotal = false,
  filters: import('./schemas').EntitySearchFilter[] = [],
) => {
  const payload = searchEntitiesRequestSchema.parse({
    blueprint: {
      code: blueprint,
      ...(version === undefined ? {} : { version }),
    },
    query,
    filters,
    relationship_tree_facets: relationshipTreeFacets,
    sort,
    ...(includeTotal ? { include_total: true } : {}),
    page: { size: 25, cursor },
  });
  return request('/api/v1/entities/search', entitySearchResponseSchema, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(payload),
    ...(signal === undefined ? {} : { signal }),
  });
};

export const getRelationshipTreeFacetChildren = (
  input: {
    blueprint: { code: string; version?: number };
    query?: string;
    source_relationship_field: string;
    hierarchy_field?: string;
    context_id: string;
    parent_id?: string;
    cursor?: string | null;
    selected_target_ids?: string[];
  },
  signal?: AbortSignal,
) =>
  request(
    '/api/v1/entities/facets/relationship-tree/children',
    relationshipTreeFacetChildrenResponseSchema,
    {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(input),
      ...(signal === undefined ? {} : { signal }),
    },
  );

export const listEntityBlueprints = (signal?: AbortSignal) =>
  request(
    '/api/blueprints',
    z.array(blueprintSchema),
    signal === undefined ? undefined : { signal },
  );
export const getEntityPreview = (id: string, signal?: AbortSignal) =>
  request(
    `/api/entities/${encodeURIComponent(uuidSchema.parse(id))}/preview?relationship_depth=0&relationship_limit=1`,
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
export const getEntityChanges = (id: string) =>
  request(
    `/api/entities/${encodeURIComponent(uuidSchema.parse(id))}/changes`,
    z.array(entityAuditChangeSchema),
  );
export const getEntityForm = (id: string) => {
  const entityId = uuidSchema.parse(id);
  return request(
    `/api/v1/entities/${encodeURIComponent(entityId)}`,
    entityFormResponseSchema,
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

export const listPublicationChannels = () =>
  request('/api/publication-channels', z.array(publicationChannelSchema));
export const getEntityPublications = (id: string) =>
  request(
    `/api/v1/entities/${encodeURIComponent(uuidSchema.parse(id))}/publications`,
    z.array(entityPublicationStatusSchema),
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
  const payload = migrateEntityRequestSchema.parse(input);
  return request(
    `/api/v1/entities/${encodeURIComponent(entityId)}/blueprint-migration`,
    entitySchema,
    {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(payload),
    },
  );
};
