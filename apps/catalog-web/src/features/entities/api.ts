import { z } from 'zod';
import {
  attributeContextSchema,
  blueprintSchema,
  blueprintWithAttributesSchema,
  createAttributeContextSchema,
  createEntityRequestSchema,
  entityHierarchySchema,
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
  AttributeContext,
  Blueprint,
  BlueprintWithAttributes,
  ComponentReference,
  Entity,
  EntityFormResponse,
  EntityMigrationPreview,
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

const apiErrorSchema = z.object({
  error: z.object({ code: z.string(), message: z.string() }),
});

export class ApiRequestError extends Error {
  status: number;
  code?: string;

  constructor(status: number, message: string, code?: string) {
    super(message);
    this.name = 'ApiRequestError';
    this.status = status;
    this.code = code;
  }
}

const request = async <T>(
  path: string,
  schema: z.ZodType<T>,
  init?: RequestInit,
): Promise<T> => {
  const response = await fetch(path, init);
  if (!response.ok) {
    const body = await response.json().catch(() => undefined);
    const error = apiErrorSchema.safeParse(body);
    throw new ApiRequestError(
      response.status,
      error.success
        ? error.data.error.message
        : `Request failed (${response.status})`,
      error.success ? error.data.error.code : undefined,
    );
  }
  const result = schema.safeParse(await response.json());
  if (!result.success)
    throw new Error(`Invalid API response: ${z.prettifyError(result.error)}`);
  return result.data;
};

export const searchEntities = (
  blueprint: string,
  version: number | undefined,
  query: string,
  cursor: string | null = null,
  relationshipTreeFacet?: {
    source_relationship_field: string;
    hierarchy_field: string;
    context_id: string;
    selected_target_ids: string[];
  },
) => {
  const payload = searchEntitiesRequestSchema.parse({
    blueprint: {
      code: blueprint,
      ...(version === undefined ? {} : { version }),
    },
    query,
    filters: [],
    relationship_tree_facet: relationshipTreeFacet,
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
  hierarchy_field: string;
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
export const listContexts = () =>
  request('/api/contexts', z.array(attributeContextSchema));
export const createContext = (
  code: string,
  data: Record<string, unknown>,
  parentId: string,
) => {
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
) =>
  request(
    `/api/contexts/id/${encodeURIComponent(id)}`,
    attributeContextSchema,
    {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ data, parent_id: parentId }),
    },
  );
export const deleteContext = async (id: string) => {
  const response = await fetch(`/api/contexts/id/${encodeURIComponent(id)}`, {
    method: 'DELETE',
  });
  if (!response.ok) throw new Error(`Request failed (${response.status})`);
};
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
