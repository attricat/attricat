import { z } from 'zod';
import { attributeContextSchema } from '../contexts/api';
import { fileMetadataSchema } from '../files/schemas';
import { attributeValueKinds, attributeValueTypes } from './valueTypes';

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
  extensionLayout: 'extension_layout',
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
  | {
      type: 'table';
      fields: string[];
      columns?: {
        field: string;
        label?: string | null;
        renderer?: ComponentReference | null;
      }[];
      component?: ComponentReference | null;
    }
  | {
      type: 'extension_layout';
      version: 1;
      outlets: Record<string, { order: string[]; hidden: string[] }>;
    }
  | Exclude<
      ViewNode,
      {
        type:
          | 'heading'
          | 'text'
          | 'divider'
          | 'field'
          | 'relationship_list'
          | 'incoming_relationship_list'
          | 'extension_layout';
      }
    >;

export const uuidSchema = z.uuid();
const jsonObjectSchema = z.record(z.string(), z.unknown());
const jsonSchemaSchema = z.union([jsonObjectSchema, z.boolean()]);
const componentReferenceSchema = z.object({
  id: z.string().regex(/^[a-z][a-z0-9_-]*(\.[a-z][a-z0-9_-]*)*$/),
  // Rust stores component protocol versions as u32; reject values that would
  // otherwise overflow the API contract.
  version: z.number().int().positive().max(4_294_967_295),
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
      fields: z
        .array(z.string())
        .nullish()
        .transform((fields) => fields ?? []),
      columns: z
        .array(
          z.object({
            field: z.string(),
            label: z.string().nullable().optional(),
            renderer: componentReferenceSchema.nullish(),
          }),
        )
        .nullish()
        .transform((columns) => columns ?? undefined),
      component: componentReferenceSchema.nullish(),
    }),
    z.object({
      type: z.literal(viewBlockTypes.extensionLayout),
      version: z.literal(1),
      outlets: z.record(
        z.string(),
        z.object({ order: z.array(z.string()), hidden: z.array(z.string()) }),
      ),
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
export const attributeSchema = z
  .object({
    code: z.string(),
    value_type: valueTypeSchema,
    target_blueprint_code: z.string().nullable().optional(),
    cardinality: z.enum(['one', 'many']).nullable().optional(),
    target_cardinality: z.enum(['one', 'many']).nullable().optional(),
    context_fallback: z.enum(['default', 'none']).optional(),
    context_editable: z.enum(['all', 'default']).optional(),
    readonly: z.boolean().optional(),
    tags: z.array(z.string()).optional(),
    value_schema: jsonSchemaSchema.nullish(),
    extension_type: z
      .object({
        provider: z.string(),
        type: z.string(),
        version: z.string(),
        primitive: z.string(),
        available: z.boolean().optional(),
      })
      .passthrough()
      .nullable()
      .optional(),
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
    id: uuidSchema,
    code: z.string(),
    name: z.string(),
    version: z.number().int().positive(),
    status: z.enum(['draft', 'published']),
    views: viewsSchema.default({}),
    entity_schema: jsonSchemaSchema.nullish(),
  })
  .passthrough();
export const blueprintWithAttributesSchema = z.object({
  blueprint: blueprintSchema,
  attributes: z.array(attributeSchema),
  table_path_attributes: z
    .array(
      z.object({
        code: z.string(),
        value_type: valueTypeSchema,
        sortable: z.boolean(),
      }),
    )
    .default([]),
});
const scalarValueSchema = z.json();
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
export const entityPublicationStatusSchema = z.object({
  context_id: uuidSchema,
  context_code: z.string(),
  status: z.enum(['not_published', 'published']),
  published_at: z.string().datetime().nullable(),
  published_by_user_id: uuidSchema.nullable(),
});
export const entityIdentitySchema = z.object({
  id: uuidSchema,
  blueprint_id: uuidSchema,
  blueprint_version: z.number().int().positive(),
  is_sample: z.boolean(),
});
export const entitySchema = entityIdentitySchema
  .extend({
    system_tags: z.array(z.string()).optional(),
    system_metadata: jsonObjectSchema.optional(),
  })
  .passthrough();
const entityItemSchema = z.object({
  id: uuidSchema,
  blueprint_version: z.number().int().positive(),
  schema_outdated: z.boolean(),
  is_sample: z.boolean(),
  display: z.record(z.string(), z.string()),
  preview: z.record(z.string(), jsonObjectSchema),
  table_values: z.record(z.string(), z.array(z.unknown())).default({}),
  related: z
    .record(
      z.string(),
      z.array(
        z.object({
          id: uuidSchema,
          blueprint_id: uuidSchema,
          blueprint_version: z.number().int().positive(),
          is_sample: z.boolean(),
          relationship_context_id: uuidSchema,
          relationship_context_code: z.string(),
          display: z.record(z.string(), z.string()),
          preview: z.record(z.string(), jsonObjectSchema),
        }),
      ),
    )
    .optional(),
  match_explanations: z
    .array(
      z.object({
        term: z.string(),
        matching_entity_id: uuidSchema,
        matching_attribute_code: z.string().nullable(),
        traversal_depth: z.number().int().nonnegative(),
        relationship_path: z.array(
          z.object({
            source_entity_id: uuidSchema,
            attribute_code: z.string(),
            target_entity_id: uuidSchema,
          }),
        ),
      }),
    )
    .default([]),
});
const entityContextSchema = z.record(z.string(), jsonObjectSchema);
export const entityPreviewResponseSchema = z.object({
  entity: entityIdentitySchema,
  context: entityContextSchema,
});
const resolvedPreviewValueSchema = z.union([
  scalarValueSchema,
  jsonObjectSchema,
  z.array(fileMetadataSchema),
]);
const resolvedEntityPreviewSchema = z.object({
  entity: entityIdentitySchema,
  requested_context: attributeContextSchema,
  values: z.record(
    z.string(),
    z.object({
      value: resolvedPreviewValueSchema,
      source_context: z.object({ id: uuidSchema, code: z.string() }),
    }),
  ),
  reusable_attributes: z.array(attributeSchema).default([]),
  reusable_values: z
    .record(
      z.string(),
      z.object({
        value: resolvedPreviewValueSchema,
        source_context: z.object({ id: uuidSchema, code: z.string() }),
      }),
    )
    .default({}),
});
const resultVersionScopeSchema = z.discriminatedUnion('kind', [
  z.object({ kind: z.literal('empty') }),
  z.object({ kind: z.literal('single'), version: z.number().int().positive() }),
  z.object({ kind: z.literal('multiple') }),
]);
const entitySearchResponseSchema = z.object({
  blueprint: blueprintWithAttributesSchema,
  items: z.array(entityItemSchema),
  next_cursor: z.string().nullable(),
  total_count: z.number().int().nonnegative().nullable().default(null),
  total_count_capped: z.boolean().default(false),
  result_version_scope: resultVersionScopeSchema.default({ kind: 'empty' }),
  hidden_outdated_count: z
    .number()
    .int()
    .nonnegative()
    .nullable()
    .default(null),
  hidden_outdated_count_capped: z.boolean().default(false),
});
const incomingRelationshipItemSchema = z.object({
  id: uuidSchema,
  blueprint_code: z.string(),
  blueprint_version: z.number().int().positive(),
  is_sample: z.boolean(),
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
export const entityAuditChangeSchema = z.object({
  audit_event_id: uuidSchema,
  occurred_at: z.string(),
  actor_user_id: uuidSchema.nullable(),
  actor_display_name: z.string().nullable(),
  actor_email: z.string().nullable(),
  executor_type: z.string(),
  agent_run_id: uuidSchema.nullable(),
  approval_decision: z.string().nullable(),
  approved_by_user_id: uuidSchema.nullable(),
  approved_by_display_name: z.string().nullable(),
  attribute_id: uuidSchema,
  attribute_code: z.string(),
  context_id: uuidSchema.nullable(),
  context_code: z.string().nullable(),
  change_kind: z.enum([
    'set',
    'replace',
    'remove',
    'relationship_add',
    'relationship_remove',
    'restore',
  ]),
  before_value: z.unknown().nullable(),
  after_value: z.unknown().nullable(),
});
const reusableEntityAttributeSchema = attributeSchema.extend({
  attachment_id: uuidSchema,
  attribute_id: uuidSchema,
  definition_id: uuidSchema,
  revision_id: uuidSchema,
  namespace: z.string(),
  name: z.string(),
  version: z.number().int().positive(),
  searchable: z.boolean(),
  facetable: z.boolean(),
  position: z.number().int(),
});
const entityFormResponseSchema = z.object({
  entity: entitySchema,
  blueprint: blueprintWithAttributesSchema,
  values: z.array(formAttributeValueSchema),
  reusable_attributes: z.array(reusableEntityAttributeSchema).default([]),
  reusable_values: z.array(formAttributeValueSchema).default([]),
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
  values: z.array(formAttributeValueSchema),
  status: z.enum(['ready', 'needs_input', 'blocked']),
  issues: z.array(migrationIssueSchema),
});
export const attributeFilterOperatorSchema = z.enum([
  'eq',
  'contains',
  'starts_with',
  'gt',
  'gte',
  'lt',
  'lte',
]);
export const entitySearchFilterSchema = z.object({
  field: z.string().min(1),
  operator: attributeFilterOperatorSchema,
  value: z.union([z.string(), z.number().finite(), z.boolean()]),
});
export const searchEntitiesRequestSchema = z.object({
  blueprint: z.object({
    code: z.string().min(1),
    version: z.number().int().positive().optional(),
  }),
  query: z.string(),
  filters: z.array(entitySearchFilterSchema),
  relationship_filters: z
    .array(
      z.object({
        field: z.string().min(1),
        selected_target_ids: z.array(uuidSchema).min(1),
      }),
    )
    .optional(),
  system_tags: z.array(z.string()).optional(),
  sort: z
    .object({
      field: z.string().min(1),
      direction: z.enum(['asc', 'desc']),
    })
    .optional(),
  include_total: z.boolean().optional(),
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
export const smartFillEntityFormRequestSchema = z.object({
  entity_id: uuidSchema,
  context_id: uuidSchema.nullable(),
  is_default_context: z.boolean(),
  content: z.string().trim().min(1).max(32_768),
});
export const smartFillEntityFormResponseSchema = z.object({
  fields: z.record(z.string(), z.string()),
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
export type Entity = z.infer<typeof entitySchema>;
export type EntityPublicationStatus = z.infer<
  typeof entityPublicationStatusSchema
>;
export type EntityAuditChange = z.infer<typeof entityAuditChangeSchema>;
export type EntityItem = z.infer<typeof entityItemSchema>;
export type EntitySearchFilter = z.infer<typeof entitySearchFilterSchema>;
export type EntitySearchResponse = z.infer<typeof entitySearchResponseSchema>;
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
