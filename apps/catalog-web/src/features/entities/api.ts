import { z } from 'zod';
import { request } from '../../api/request';
import {
  blueprintSchema,
  blueprintWithAttributesSchema,
  createEntityRequestSchema,
  entityHierarchySchema,
  entityAuditChangeSchema,
  entityFormResponseSchema,
  entityMigrationPreviewSchema,
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
  FormAttributeValue,
  EntityItem,
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
) => {
  const payload = searchEntitiesRequestSchema.parse({
    blueprint: {
      code: blueprint,
      ...(version === undefined ? {} : { version }),
    },
    query,
    filters: [],
    relationship_tree_facets: relationshipTreeFacets,
    page: { size: 25, cursor },
  });
  return request('/api/v1/entities/search', entitySearchResponseSchema, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(payload),
  });
};

export const getRelationshipTreeFacetChildren = (input: {
  blueprint: { code: string; version?: number };
  query?: string;
  source_relationship_field: string;
  hierarchy_field?: string;
  context_id: string;
  parent_id?: string;
  cursor?: string | null;
}) =>
  request(
    '/api/v1/entities/facets/relationship-tree/children',
    relationshipTreeFacetChildrenResponseSchema,
    {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(input),
    },
  );

export const listEntityBlueprints = () =>
  request('/api/blueprints', z.array(blueprintSchema));
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
export const getBlueprintByCode = (code: string, version?: number) => {
  const input = getBlueprintRequestSchema.parse({ code, version });
  const path = `/api/blueprints/by-code/${encodeURIComponent(input.code)}${input.version === undefined ? '' : `/versions/${input.version}`}`;
  return request(path, blueprintWithAttributesSchema);
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
