import { z } from 'zod';

const uuidSchema = z.uuid();
const jsonObjectSchema = z.record(z.string(), z.unknown());
const valueTypeSchema = z.enum([
  'string',
  'number',
  'integer',
  'boolean',
  'date',
  'datetime',
  'time',
  'relationship',
]);

export const attributeSchema = z
  .object({
    code: z.string(),
    value_type: valueTypeSchema,
    target_blueprint_code: z.string().nullable().optional(),
  })
  .passthrough();
export const blueprintSchema = z
  .object({
    code: z.string(),
    name: z.string(),
    version: z.number().int().positive(),
    display: jsonObjectSchema,
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
    kind: z.literal('scalar'),
    attribute_code: z.string().min(1),
    value: scalarValueSchema,
  }),
  z.object({
    kind: z.literal('relationship'),
    attribute_code: z.string().min(1),
    target_entity_id: uuidSchema,
  }),
]);
export const relationshipTargetsSchema = z.object({
  attribute_code: z.string().min(1),
  target_entity_ids: z.array(uuidSchema),
});
export const entitySchema = z.object({ id: uuidSchema }).passthrough();
const entityItemSchema = z.object({
  id: uuidSchema,
  display: z.record(z.string(), z.string()),
  preview: jsonObjectSchema,
});
const entityPreviewSchema = z.record(z.string(), jsonObjectSchema);
const entitySearchResponseSchema = z.object({
  blueprint: blueprintWithAttributesSchema,
  items: z.array(entityItemSchema),
  next_cursor: z.string().nullable(),
});
const entityFormResponseSchema = z.object({
  entity: entitySchema,
  blueprint: blueprintWithAttributesSchema,
  values: z.array(newAttributeValueSchema),
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
});

export type Attribute = z.infer<typeof attributeSchema>;
export type Blueprint = z.infer<typeof blueprintSchema>;
export type BlueprintWithAttributes = z.infer<
  typeof blueprintWithAttributesSchema
>;
export type NewAttributeValue = z.infer<typeof newAttributeValueSchema>;
export type RelationshipTargets = z.infer<typeof relationshipTargetsSchema>;
export type Entity = z.infer<typeof entitySchema>;
export type EntityItem = z.infer<typeof entityItemSchema>;
export type EntitySearchResponse = z.infer<typeof entitySearchResponseSchema>;
export type EntityFormResponse = z.infer<typeof entityFormResponseSchema>;
export type EntityPreview = z.infer<typeof entityPreviewSchema>;

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

export const getEntityPreview = (id: string) => {
  const entityId = uuidSchema.parse(id);
  return request(
    `/api/entities/${encodeURIComponent(entityId)}/projections/preview`,
    entityPreviewSchema,
  );
};

export const getBlueprintByCode = (code: string, version?: number) => {
  const input = getBlueprintRequestSchema.parse({ code, version });
  const path = `/api/blueprints/by-code/${encodeURIComponent(input.code)}${
    input.version === undefined ? '' : `/versions/${input.version}`
  }`;
  return request(path, blueprintWithAttributesSchema);
};

export const getEntityForm = (id: string) => {
  const entityId = uuidSchema.parse(id);
  return request(
    `/api/v1/entities/${encodeURIComponent(entityId)}/form`,
    entityFormResponseSchema,
  );
};

export const createEntity = (input: z.input<typeof createEntityRequestSchema>) => {
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
