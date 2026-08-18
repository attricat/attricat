import { z } from 'zod';
import { attributeValueKinds, attributeValueTypes } from './value-types';

const uuidSchema = z.uuid();
const jsonObjectSchema = z.record(z.string(), z.unknown());
const componentReferenceSchema = z.object({
  id: z.string().regex(/^[a-z][a-z0-9_]*(\.[a-z][a-z0-9_]*)*$/),
  version: z.number().int().positive(),
  props: jsonObjectSchema.nullish().transform((props) => props ?? {}),
});
const viewNodeSchema: z.ZodType<ViewNode> = z.lazy(() =>
  z.discriminatedUnion('type', [
    z.object({ type: z.literal('stack'), children: z.array(viewNodeSchema), component: componentReferenceSchema.nullish() }),
    z.object({ type: z.literal('grid'), children: z.array(viewNodeSchema), component: componentReferenceSchema.nullish() }),
    z.object({ type: z.literal('section'), children: z.array(viewNodeSchema), component: componentReferenceSchema.nullish() }),
    z.object({ type: z.literal('tabs'), tabs: z.array(z.object({ label: z.string(), children: z.array(viewNodeSchema) })), component: componentReferenceSchema.nullish() }),
    z.object({ type: z.literal('accordion'), sections: z.array(z.object({ label: z.string(), children: z.array(viewNodeSchema) })), component: componentReferenceSchema.nullish() }),
    z.object({ type: z.literal('heading'), text: z.string(), component: componentReferenceSchema.nullish() }),
    z.object({ type: z.literal('text'), text: z.string(), component: componentReferenceSchema.nullish() }),
    z.object({ type: z.literal('divider'), component: componentReferenceSchema.nullish() }),
    z.object({ type: z.literal('field'), field: z.string(), component: componentReferenceSchema.nullish() }),
    z.object({ type: z.literal('relationship_list'), field: z.string(), component: componentReferenceSchema.nullish() }),
  ]),
);
const viewDefinitionSchema: z.ZodType<ViewDefinition> = z.lazy(() =>
  z.union([
    z.object({ type: z.literal('table'), fields: z.array(z.string()), component: componentReferenceSchema.nullish() }),
    z.object({ type: z.literal('stack'), children: z.array(viewNodeSchema), component: componentReferenceSchema.nullish() }),
    z.object({ type: z.literal('grid'), children: z.array(viewNodeSchema), component: componentReferenceSchema.nullish() }),
    z.object({ type: z.literal('section'), children: z.array(viewNodeSchema), component: componentReferenceSchema.nullish() }),
    z.object({ type: z.literal('tabs'), tabs: z.array(z.object({ label: z.string(), children: z.array(viewNodeSchema) })), component: componentReferenceSchema.nullish() }),
    z.object({ type: z.literal('accordion'), sections: z.array(z.object({ label: z.string(), children: z.array(viewNodeSchema) })), component: componentReferenceSchema.nullish() }),
  ]),
);
const viewsSchema = z.record(z.string(), viewDefinitionSchema);
const valueTypeSchema = z.enum(attributeValueTypes);
export const contextCodeSchema = z
  .string()
  .regex(/^[A-Za-z0-9_-]+$/, 'Use only letters, numbers, hyphens, and underscores');

export const attributeSchema = z
  .object({
    code: z.string(),
    value_type: valueTypeSchema,
    target_blueprint_code: z.string().nullable().optional(),
    context_fallback: z.enum(['default', 'none']).optional(),
    context_editable: z.enum(['all', 'default']).optional(),
  })
  .passthrough();
export const blueprintSchema = z
  .object({
    code: z.string(),
    name: z.string(),
    version: z.number().int().positive(),
    display: jsonObjectSchema,
    views: viewsSchema.default({}),
  })
  .passthrough();
export const blueprintWithAttributesSchema = z.object({
  blueprint: blueprintSchema,
  attributes: z.array(attributeSchema),
});
const scalarValueSchema = z.union([
  z.string(),
  z.number().finite(),
  z.boolean(),
  z.object({ time: z.string(), time_zone: z.string() }),
]);
export const newAttributeValueSchema = z.discriminatedUnion('kind', [
  z.object({
    kind: z.literal(attributeValueKinds.scalar),
    attribute_code: z.string().min(1),
    context_id: uuidSchema.nullable().optional(),
    value: scalarValueSchema,
  }),
  z.object({
    kind: z.literal(attributeValueKinds.relationship),
    attribute_code: z.string().min(1),
    context_id: uuidSchema.nullable().optional(),
    target_entity_id: uuidSchema,
  }),
]);
export const relationshipTargetsSchema = z.object({
  attribute_code: z.string().min(1),
  context_id: uuidSchema.nullable().optional(),
  target_entity_ids: z.array(uuidSchema),
});
const attributeValueSelectorSchema = z.object({
  attribute_code: z.string().min(1),
  context_id: uuidSchema.nullable(),
});
export const attributeContextSchema = z.object({
  id: uuidSchema,
  code: contextCodeSchema,
  data: jsonObjectSchema,
  parent_id: uuidSchema.nullable(),
});
export const createAttributeContextSchema = z.object({
  code: contextCodeSchema,
  data: jsonObjectSchema,
  parent_id: uuidSchema,
});
export const entitySchema = z.object({
  id: uuidSchema,
  blueprint_id: uuidSchema.optional(),
  blueprint_version: z.number().int().positive().optional(),
}).passthrough();
const entityItemSchema = z.object({
  id: uuidSchema,
  blueprint_version: z.number().int().positive(),
  schema_outdated: z.boolean(),
  display: z.record(z.string(), z.string()),
  preview: z.record(z.string(), jsonObjectSchema),
});
const entityContextSchema = z.record(z.string(), jsonObjectSchema);
const entityPreviewSchema = z.object({
  entity: entitySchema,
  context: entityContextSchema,
});
const resolvedPreviewValueSchema = z.union([scalarValueSchema, jsonObjectSchema]);
const resolvedEntityPreviewSchema = z.object({
  requested_context: attributeContextSchema,
  values: z.record(
    z.string(),
    z.object({
      value: resolvedPreviewValueSchema,
      source_context: z.object({ id: uuidSchema, code: z.string() }),
    }),
  ),
});
const entitySearchResponseSchema = z.object({
  blueprint: blueprintWithAttributesSchema,
  items: z.array(entityItemSchema),
  next_cursor: z.string().nullable(),
});
const entityFormResponseSchema = z.object({
  entity: entitySchema,
  blueprint: blueprintWithAttributesSchema,
  values: z.array(newAttributeValueSchema),
  context: entityContextSchema,
});
const searchEntitiesRequestSchema = z.object({
  blueprint: z.object({
    code: z.string().min(1),
    version: z.number().int().positive().optional(),
  }),
  query: z.string(),
  filters: z.array(z.never()),
  page: z.object({ size: z.number().int().positive(), cursor: z.null() }),
});
const getBlueprintRequestSchema = z.object({
  code: z.string().min(1),
  version: z.number().int().positive().optional(),
});
const getBlueprintRevisionRequestSchema = z.object({
  id: uuidSchema,
  version: z.number().int().positive(),
});
const createEntityRequestSchema = z.object({
  blueprint: z.object({
    code: z.string().min(1),
    version: z.number().int().positive().optional(),
  }),
  values: z.array(newAttributeValueSchema),
});
const updateEntityRequestSchema = z.object({
  values: z.array(newAttributeValueSchema),
  relationships: z.array(relationshipTargetsSchema),
  remove_values: z.array(attributeValueSelectorSchema).default([]),
});

export type Attribute = z.infer<typeof attributeSchema>;
export type ViewNode =
  | { type: 'stack' | 'grid' | 'section'; children: ViewNode[]; component?: ComponentReference | null }
  | { type: 'tabs'; tabs: { label: string; children: ViewNode[] }[]; component?: ComponentReference | null }
  | { type: 'accordion'; sections: { label: string; children: ViewNode[] }[]; component?: ComponentReference | null }
  | { type: 'heading' | 'text'; text: string; component?: ComponentReference | null }
  | { type: 'divider'; component?: ComponentReference | null }
  | { type: 'field' | 'relationship_list'; field: string; component?: ComponentReference | null };
export type ViewDefinition =
  | { type: 'table'; fields: string[]; component?: ComponentReference | null }
  | Exclude<ViewNode, { type: 'heading' | 'text' | 'divider' | 'field' | 'relationship_list' }>;
export type ComponentReference = {
  id: string;
  version: number;
  props: Record<string, unknown>;
};
export type Blueprint = z.infer<typeof blueprintSchema>;
export type BlueprintWithAttributes = z.infer<
  typeof blueprintWithAttributesSchema
>;
export type NewAttributeValue = z.infer<typeof newAttributeValueSchema>;
export type RelationshipTargets = z.infer<typeof relationshipTargetsSchema>;
export type AttributeContext = z.infer<typeof attributeContextSchema>;
export type Entity = z.infer<typeof entitySchema>;
export type EntityItem = z.infer<typeof entityItemSchema>;
export type EntitySearchResponse = z.infer<typeof entitySearchResponseSchema>;
export type EntityFormResponse = z.infer<typeof entityFormResponseSchema>;
export type EntityPreview = z.infer<typeof entityPreviewSchema>;
export type ResolvedEntityPreview = z.infer<typeof resolvedEntityPreviewSchema>;

const request = async <T>(
  path: string,
  schema: z.ZodType<T>,
  init?: RequestInit,
): Promise<T> => {
  const response = await fetch(path, init);
  if (!response.ok) throw new Error(`Request failed (${response.status})`);
  const result = schema.safeParse(await response.json());
  if (!result.success) {
    throw new Error(`Invalid API response: ${z.prettifyError(result.error)}`);
  }
  return result.data;
};

export const searchEntities = (
  blueprint: string,
  version: number | undefined,
  query: string,
) => {
  const payload = searchEntitiesRequestSchema.parse({
    blueprint: {
      code: blueprint,
      ...(version === undefined ? {} : { version }),
    },
    query,
    filters: [],
    page: { size: 25, cursor: null },
  });
  return request('/api/v1/entities/search', entitySearchResponseSchema, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(payload),
  });
};

export const listEntityBlueprints = () => {
  return request('/api/blueprints', z.array(blueprintSchema));
};

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
  request(`/api/contexts/id/${encodeURIComponent(id)}`, attributeContextSchema, {
    method: 'PUT',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ data, parent_id: parentId }),
  });

export const deleteContext = async (id: string) => {
  const response = await fetch(`/api/contexts/id/${encodeURIComponent(id)}`, {
    method: 'DELETE',
  });
  if (!response.ok) throw new Error(`Request failed (${response.status})`);
};

export const getEntityPreview = (id: string) => {
  const entityId = uuidSchema.parse(id);
  return request(
    `/api/entities/${encodeURIComponent(entityId)}/preview`,
    entityPreviewSchema,
  );
};

export const getResolvedEntityPreview = (id: string, contextId: string) =>
  request(
    `/api/entities/${encodeURIComponent(uuidSchema.parse(id))}/resolved-preview?context_id=${encodeURIComponent(uuidSchema.parse(contextId))}`,
    resolvedEntityPreviewSchema,
  );

export const getBlueprintByCode = (code: string, version?: number) => {
  const input = getBlueprintRequestSchema.parse({ code, version });
  const path = `/api/blueprints/by-code/${encodeURIComponent(input.code)}${
    input.version === undefined ? '' : `/versions/${input.version}`
  }`;
  return request(path, blueprintWithAttributesSchema);
};

export const getBlueprintRevision = (id: string, version: number) => {
  const input = getBlueprintRevisionRequestSchema.parse({ id, version });
  return request(
    `/api/blueprints/${encodeURIComponent(input.id)}/versions/${input.version}`,
    blueprintWithAttributesSchema,
  );
};

export const getEntityForm = (id: string) => {
  const entityId = uuidSchema.parse(id);
  return request(
    `/api/v1/entities/${encodeURIComponent(entityId)}`,
    entityFormResponseSchema,
  );
};

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
