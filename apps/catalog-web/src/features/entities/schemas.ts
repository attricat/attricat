import { z } from 'zod';
import { fileMetadataSchema } from '../files/schemas';
import { attributeValueKinds, attributeValueTypes } from './value-types';

export const viewBlockTypes = {
  dropdownOption: 'dropdown_option',
  stack: 'stack',
  grid: 'grid',
  section: 'section',
  tabs: 'tabs',
  accordion: 'accordion',
  heading: 'heading',
  text: 'text',
  divider: 'divider',
  field: 'field',
  relationshipList: 'relationship_list',
  incomingRelationshipList: 'incoming_relationship_list',
  table: 'table',
} as const;

export type ComponentReference = {
  id: string;
  version: number;
  props: Record<string, unknown>;
};
export type ViewNode =
  | {
      type: 'stack' | 'grid' | 'section';
      children: ViewNode[];
      component?: ComponentReference | null;
    }
  | {
      type: 'tabs';
      tabs: { label: string; children: ViewNode[] }[];
      component?: ComponentReference | null;
    }
  | {
      type: 'accordion';
      sections: { label: string; children: ViewNode[] }[];
      component?: ComponentReference | null;
    }
  | {
      type: 'heading' | 'text';
      text: string;
      component?: ComponentReference | null;
    }
  | { type: 'divider'; component?: ComponentReference | null }
  | {
      type: 'field' | 'relationship_list';
      field: string;
      component?: ComponentReference | null;
    }
  | {
      type: 'incoming_relationship_list';
      label: string;
      relationships: { source_blueprint: string; field: string }[];
      page_size: number;
      component?: ComponentReference | null;
    };
export type ViewDefinition =
  | {
      type: 'dropdown_option';
      fields: string[];
      separator?: string;
    }
  | { type: 'table'; fields: string[]; component?: ComponentReference | null }
  | Exclude<
      ViewNode,
      {
        type:
          | 'heading'
          | 'text'
          | 'divider'
          | 'field'
          | 'relationship_list'
          | 'incoming_relationship_list';
      }
    >;

export const uuidSchema = z.uuid();
const jsonObjectSchema = z.record(z.string(), z.unknown());
const jsonSchemaSchema = z.union([jsonObjectSchema, z.boolean()]);
const componentReferenceSchema = z.object({
  id: z.string().regex(/^[a-z][a-z0-9_]*(\.[a-z][a-z0-9_]*)*$/),
  version: z.number().int().positive(),
  props: jsonObjectSchema.nullish().transform((props) => props ?? {}),
});
const viewNodeSchema: z.ZodType<ViewNode> = z.lazy(() =>
  z.discriminatedUnion('type', [
    z.object({
      type: z.literal(viewBlockTypes.stack),
      children: z.array(viewNodeSchema),
      component: componentReferenceSchema.nullish(),
    }),
    z.object({
      type: z.literal(viewBlockTypes.grid),
      children: z.array(viewNodeSchema),
      component: componentReferenceSchema.nullish(),
    }),
    z.object({
      type: z.literal(viewBlockTypes.section),
      children: z.array(viewNodeSchema),
      component: componentReferenceSchema.nullish(),
    }),
    z.object({
      type: z.literal(viewBlockTypes.tabs),
      tabs: z.array(
        z.object({ label: z.string(), children: z.array(viewNodeSchema) }),
      ),
      component: componentReferenceSchema.nullish(),
    }),
    z.object({
      type: z.literal(viewBlockTypes.accordion),
      sections: z.array(
        z.object({ label: z.string(), children: z.array(viewNodeSchema) }),
      ),
      component: componentReferenceSchema.nullish(),
    }),
    z.object({
      type: z.literal(viewBlockTypes.heading),
      text: z.string(),
      component: componentReferenceSchema.nullish(),
    }),
    z.object({
      type: z.literal(viewBlockTypes.text),
      text: z.string(),
      component: componentReferenceSchema.nullish(),
    }),
    z.object({
      type: z.literal(viewBlockTypes.divider),
      component: componentReferenceSchema.nullish(),
    }),
    z.object({
      type: z.literal(viewBlockTypes.field),
      field: z.string(),
      component: componentReferenceSchema.nullish(),
    }),
    z.object({
      type: z.literal(viewBlockTypes.relationshipList),
      field: z.string(),
      component: componentReferenceSchema.nullish(),
    }),
    z.object({
      type: z.literal(viewBlockTypes.incomingRelationshipList),
      label: z.string(),
      relationships: z.array(
        z.object({ source_blueprint: z.string(), field: z.string() }),
      ),
      page_size: z.number().int().positive(),
      component: componentReferenceSchema.nullish(),
    }),
  ]),
);
const viewDefinitionSchema: z.ZodType<ViewDefinition> = z.lazy(() =>
  z.union([
    z.object({
      type: z.literal(viewBlockTypes.dropdownOption),
      fields: z.array(z.string()),
      separator: z.string().optional(),
    }),
    z.object({
      type: z.literal(viewBlockTypes.table),
      fields: z.array(z.string()),
      component: componentReferenceSchema.nullish(),
    }),
    z.object({
      type: z.literal(viewBlockTypes.stack),
      children: z.array(viewNodeSchema),
      component: componentReferenceSchema.nullish(),
    }),
    z.object({
      type: z.literal(viewBlockTypes.grid),
      children: z.array(viewNodeSchema),
      component: componentReferenceSchema.nullish(),
    }),
    z.object({
      type: z.literal(viewBlockTypes.section),
      children: z.array(viewNodeSchema),
      component: componentReferenceSchema.nullish(),
    }),
    z.object({
      type: z.literal(viewBlockTypes.tabs),
      tabs: z.array(
        z.object({ label: z.string(), children: z.array(viewNodeSchema) }),
      ),
      component: componentReferenceSchema.nullish(),
    }),
    z.object({
      type: z.literal(viewBlockTypes.accordion),
      sections: z.array(
        z.object({ label: z.string(), children: z.array(viewNodeSchema) }),
      ),
      component: componentReferenceSchema.nullish(),
    }),
  ]),
);
const viewsSchema = z.record(z.string(), viewDefinitionSchema);
const valueTypeSchema = z.enum(attributeValueTypes);
export const contextCodeSchema = z
  .string()
  .regex(
    /^[A-Za-z0-9_-]+$/,
    'Use only letters, numbers, hyphens, and underscores',
  );
export const attributeSchema = z
  .object({
    code: z.string(),
    value_type: valueTypeSchema,
    target_blueprint_code: z.string().nullable().optional(),
    context_fallback: z.enum(['default', 'none']).optional(),
    context_editable: z.enum(['all', 'default']).optional(),
    value_schema: jsonSchemaSchema.nullish(),
    file_policy: z
      .object({
        cardinality: z.enum(['one', 'many']),
        ordered: z.boolean(),
        allowed_mime_groups: z.array(z.string()),
        allowed_extensions: z.array(z.string()),
        max_bytes: z.number().int().positive().nullable().optional(),
        purposes: z.array(z.string()),
        image_only: z.boolean(),
      })
      .nullable()
      .optional(),
  })
  .passthrough();
export const blueprintSchema = z
  .object({
    code: z.string(),
    name: z.string(),
    version: z.number().int().positive(),
    views: viewsSchema.default({}),
    entity_schema: jsonSchemaSchema.nullish(),
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
export const formAttributeValueSchema = z.discriminatedUnion('kind', [
  ...newAttributeValueSchema.options,
  z.object({
    kind: z.literal('file'),
    attribute_code: z.string().min(1),
    context_id: uuidSchema.nullable().optional(),
    files: z.array(fileMetadataSchema),
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
export const entitySchema = z
  .object({
    id: uuidSchema,
    blueprint_id: uuidSchema.optional(),
    blueprint_version: z.number().int().positive().optional(),
    system_tags: z.array(z.string()).optional(),
    system_metadata: jsonObjectSchema.optional(),
  })
  .passthrough();
const entityItemSchema = z.object({
  id: uuidSchema,
  blueprint_version: z.number().int().positive(),
  schema_outdated: z.boolean(),
  display: z.record(z.string(), z.string()),
  preview: z.record(z.string(), jsonObjectSchema),
});
const entityContextSchema = z.record(z.string(), jsonObjectSchema);
const resolvedPreviewValueSchema = z.union([
  scalarValueSchema,
  jsonObjectSchema,
]);
const resolvedEntityPreviewSchema = z.object({
  entity: entitySchema,
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
export const relationshipTreeFacetChildrenResponseSchema = z.object({
  items: z.array(
    z.object({
      id: uuidSchema,
      display: z.string(),
      count: z.number().int().nonnegative(),
      has_children: z.boolean(),
    }),
  ),
  next_cursor: uuidSchema.nullable(),
});
const incomingRelationshipItemSchema = z.object({
  id: uuidSchema,
  blueprint_code: z.string(),
  blueprint_version: z.number().int().positive(),
  display: z.record(z.string(), z.string()),
});
export const incomingRelationshipsPageSchema = z.object({
  items: z.array(incomingRelationshipItemSchema),
  next_cursor: z.string().nullable(),
});
export const entityHierarchySchema = z.object({
  items: z.array(z.object({ id: uuidSchema, display: z.string() })),
  paths: z.array(z.array(z.object({ id: uuidSchema, display: z.string() }))),
  truncated: z.boolean(),
  multiple_parents: z.boolean(),
  cycle_detected: z.boolean(),
});
const entityFormResponseSchema = z.object({
  entity: entitySchema,
  blueprint: blueprintWithAttributesSchema,
  values: z.array(formAttributeValueSchema),
  context: entityContextSchema,
});
const migrationIssueSchema = z.object({
  attribute_code: z.string().nullable(),
  kind: z.enum([
    'removed',
    'value_type_changed',
    'relationship_target_changed',
    'attribute_schema_mismatch',
    'missing_required',
  ]),
  message: z.string(),
});
export const entityMigrationPreviewSchema = z.object({
  migration_id: uuidSchema,
  source_version: z.number().int().positive(),
  target: blueprintWithAttributesSchema,
  values: z.array(newAttributeValueSchema),
  status: z.enum(['ready', 'needs_input', 'blocked']),
  issues: z.array(migrationIssueSchema),
});
export const searchEntitiesRequestSchema = z.object({
  blueprint: z.object({
    code: z.string().min(1),
    version: z.number().int().positive().optional(),
  }),
  query: z.string(),
  filters: z.array(z.never()),
  system_tags: z.array(z.string()).optional(),
  relationship_tree_facet: z
    .object({
      source_relationship_field: z.string().min(1),
      hierarchy_field: z.string().min(1),
      context_id: uuidSchema,
      selected_target_ids: z.array(uuidSchema),
    })
    .optional(),
  page: z.object({
    size: z.number().int().positive(),
    cursor: z.string().nullable(),
  }),
});
const getBlueprintRequestSchema = z.object({
  code: z.string().min(1),
  version: z.number().int().positive().optional(),
});
const getBlueprintRevisionRequestSchema = z.object({
  id: uuidSchema,
  version: z.number().int().positive(),
});
export const createEntityRequestSchema = z.object({
  blueprint: z.object({
    code: z.string().min(1),
    version: z.number().int().positive().optional(),
  }),
  values: z.array(newAttributeValueSchema),
  system_tags: z.array(z.string()).optional(),
  system_metadata: jsonObjectSchema.optional(),
});
export const updateEntityRequestSchema = z.object({
  values: z.array(newAttributeValueSchema),
  relationships: z.array(relationshipTargetsSchema),
  remove_values: z.array(attributeValueSelectorSchema).default([]),
  system_tags: z.array(z.string()).optional(),
  system_metadata: jsonObjectSchema.optional(),
});
export const migrateEntityRequestSchema = z.object({
  migration_id: uuidSchema,
  expected_target_version: z.number().int().positive(),
  values: z.array(newAttributeValueSchema),
  relationships: z.array(relationshipTargetsSchema),
  discard_attributes: z.array(z.string()),
});

export type Attribute = z.infer<typeof attributeSchema>;
export type Blueprint = z.infer<typeof blueprintSchema>;
export type JsonSchema = z.infer<typeof jsonSchemaSchema>;
export type BlueprintWithAttributes = z.infer<
  typeof blueprintWithAttributesSchema
>;
export type NewAttributeValue = z.infer<typeof newAttributeValueSchema>;
export type FormAttributeValue = z.infer<typeof formAttributeValueSchema>;
export type RelationshipTargets = z.infer<typeof relationshipTargetsSchema>;
export type AttributeContext = z.infer<typeof attributeContextSchema>;
export type Entity = z.infer<typeof entitySchema>;
export type EntityItem = z.infer<typeof entityItemSchema>;
export type EntitySearchResponse = z.infer<typeof entitySearchResponseSchema>;
export type RelationshipTreeFacetChildrenResponse = z.infer<
  typeof relationshipTreeFacetChildrenResponseSchema
>;
export type IncomingRelationshipsPage = z.infer<
  typeof incomingRelationshipsPageSchema
>;
export type EntityFormResponse = z.infer<typeof entityFormResponseSchema>;
export type ResolvedEntityPreview = z.infer<typeof resolvedEntityPreviewSchema>;
export type EntityMigrationPreview = z.infer<
  typeof entityMigrationPreviewSchema
>;

export {
  entityFormResponseSchema,
  entitySearchResponseSchema,
  getBlueprintRequestSchema,
  getBlueprintRevisionRequestSchema,
  resolvedEntityPreviewSchema,
};
