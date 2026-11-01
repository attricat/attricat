import { z } from 'zod';
import { dataQualityFilters } from '../rules/constants';
import { checkViolationSchema } from '../../api/checkViolations';
import { attributeContextSchema } from '../contexts/schemas';
import { fileMetadataSchema } from '../files/schemas';
import { attributeValueKinds, attributeValueTypes } from './valueTypes';
import { jsonObjectSchema, viewsSchema } from '../views/schemas';
import { revisionStatusSchema } from '../../api/revisionStatus';
import {
  attributeCardinalities,
  attributeContextEditability,
  attributeContextFallbacks,
  attributeFilterOperators,
  attributeHierarchies,
  generatedSequenceResets,
  publicationStatuses,
  resultVersionScopeKinds,
  approvalEndReasons,
  approvalCauseKinds,
  statusTransitionDenialCodes,
  retentionHoldSources,
  transitionChannels,
  transitionOperations,
} from './constants';

export const uuidSchema = z.uuid();
export const recordLabelsResponseSchema = z.object({
  items: z.array(
    z.object({
      id: z.uuid(),
      blueprint_code: z.string(),
      display: z.record(z.string(), z.string()),
    }),
  ),
});
const jsonSchemaSchema = z.union([jsonObjectSchema, z.boolean()]);
const valueTypeSchema = z.enum(attributeValueTypes);
export const attributeSchema = z
  .object({
    code: z.string(),
    name: z.string().nullish(),
    /** What the attribute holds, as its blueprint describes it. */
    description: z.string().nullish(),
    value_type: valueTypeSchema,
    target_blueprint_code: z.string().nullable().optional(),
    /** Every allowed target blueprint; empty means any blueprint. */
    target_blueprint_codes: z.array(z.string()).optional(),
    cardinality: z.enum(attributeCardinalities).nullable().optional(),
    target_cardinality: z.enum(attributeCardinalities).nullable().optional(),
    hierarchy: z.enum(attributeHierarchies).nullable().optional(),
    context_fallback: z.enum(attributeContextFallbacks).optional(),
    context_editable: z.enum(attributeContextEditability).optional(),
    readonly: z.boolean().optional(),
    /** Numbered by the server on record creation; never written by clients. */
    generated: z
      .object({
        format: z.string().optional(),
        start: z.number(),
        reset: z.enum(generatedSequenceResets),
        time_zone: z.string(),
      })
      .nullish(),
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
        cardinality: z.enum(attributeCardinalities),
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
    status: revisionStatusSchema,
    views: viewsSchema.default({}),
    record_schema: jsonSchemaSchema.nullish(),
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
/** The attribute and context a value is set in; no context means the default. */
const attributeValueTargetSchema = z.object({
  attribute_code: z.string().min(1),
  context_id: uuidSchema.nullable().optional(),
});
/** A blueprint by code, at a version or its current one. */
const blueprintSelectorSchema = z.object({
  code: z.string().min(1),
  version: z.number().int().positive().optional(),
});
const newAttributeValueSchema = z.discriminatedUnion('kind', [
  attributeValueTargetSchema.extend({
    kind: z.literal(attributeValueKinds.scalar),
    value: scalarValueSchema,
  }),
  attributeValueTargetSchema.extend({
    kind: z.literal(attributeValueKinds.relationship),
    target_record_id: uuidSchema,
  }),
]);
const formAttributeValueSchema = z.discriminatedUnion('kind', [
  ...newAttributeValueSchema.options,
  attributeValueTargetSchema.extend({
    kind: z.literal(attributeValueKinds.file),
    files: z.array(fileMetadataSchema),
  }),
]);
const relationshipTargetsSchema = attributeValueTargetSchema.extend({
  target_record_ids: z.array(uuidSchema),
});
const attributeValueSelectorSchema = z.object({
  attribute_code: z.string().min(1),
  context_id: uuidSchema.nullable(),
});
export const recordPublicationStatusSchema = z.object({
  context_id: uuidSchema,
  context_code: z.string(),
  status: z.enum(publicationStatuses),
  published_at: z.string().datetime().nullable(),
  published_by_user_id: uuidSchema.nullable(),
});
export const recordPublicationReadinessSchema = z.object({
  context_id: uuidSchema,
  context_code: z.string(),
  ready: z.boolean(),
  violations: z.array(checkViolationSchema),
});
const recordIdentitySchema = z.object({
  id: uuidSchema,
  blueprint_id: uuidSchema,
  blueprint_version: z.number().int().positive(),
  is_sample: z.boolean(),
});
export const recordSchema = recordIdentitySchema
  .extend({
    updated_at: z.string().datetime({ offset: true }).optional(),
    system_tags: z.array(z.string()).optional(),
    system_metadata: jsonObjectSchema.optional(),
  })
  .passthrough();
const recordItemSchema = z.object({
  id: uuidSchema,
  blueprint_version: z.number().int().positive(),
  schema_outdated: z.boolean(),
  is_sample: z.boolean(),
  system_tags: z.array(z.string()).default([]),
  display: z.record(z.string(), z.string()),
  /** Omitted by searches that exclude previews. */
  preview: z.record(z.string(), jsonObjectSchema).default({}),
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
        matching_record_id: uuidSchema,
        matching_attribute_code: z.string().nullable(),
        traversal_depth: z.number().int().nonnegative(),
        relationship_path: z.array(
          z.object({
            source_record_id: uuidSchema,
            attribute_code: z.string(),
            target_record_id: uuidSchema,
          }),
        ),
      }),
    )
    .default([]),
});
const recordContextSchema = z.record(z.string(), jsonObjectSchema);
const resolvedPreviewValueSchema = z.union([
  scalarValueSchema,
  jsonObjectSchema,
  z.array(fileMetadataSchema),
]);
const resolvedPreviewEntrySchema = z.object({
  value: resolvedPreviewValueSchema,
  /** The context the value is set in, which may be a parent of the one asked for. */
  source_context: z.object({ id: uuidSchema, code: z.string() }),
});
const resolvedRecordPreviewSchema = z.object({
  record: recordIdentitySchema,
  requested_context: attributeContextSchema,
  values: z.record(z.string(), resolvedPreviewEntrySchema),
  reusable_attributes: z.array(attributeSchema).default([]),
  reusable_values: z.record(z.string(), resolvedPreviewEntrySchema).default({}),
});
const resultVersionScopeSchema = z.discriminatedUnion('kind', [
  z.object({ kind: z.literal(resultVersionScopeKinds.empty) }),
  z.object({
    kind: z.literal(resultVersionScopeKinds.single),
    version: z.number().int().positive(),
  }),
  z.object({ kind: z.literal(resultVersionScopeKinds.multiple) }),
]);
type ResultVersionScope = z.infer<typeof resultVersionScopeSchema>;

/** The blueprint version every result of a search shares, if there is one. */
export const singleResultVersionScope = (
  page: { result_version_scope: ResultVersionScope } | undefined,
) =>
  page?.result_version_scope.kind === resultVersionScopeKinds.single
    ? page.result_version_scope
    : undefined;

/**
 * Whether a search shows exactly its blueprint's revision, so actions on that
 * revision apply. No results still show it, which is where an import starts.
 */
export const resultsShowBlueprintRevision = (
  page:
    | {
        blueprint?: { blueprint: { version: number } };
        result_version_scope: ResultVersionScope;
      }
    | undefined,
) =>
  page !== undefined &&
  (page.result_version_scope.kind === resultVersionScopeKinds.empty ||
    singleResultVersionScope(page)?.version ===
      page.blueprint?.blueprint.version);

const recordSearchResponseSchema = z.object({
  blueprint: blueprintWithAttributesSchema,
  items: z.array(recordItemSchema),
  next_cursor: z.string().nullable(),
  total_count: z.number().int().nonnegative().nullable().default(null),
  total_count_capped: z.boolean().default(false),
  result_version_scope: resultVersionScopeSchema.default({
    kind: resultVersionScopeKinds.empty,
  }),
  hidden_outdated_count: z
    .number()
    .int()
    .nonnegative()
    .nullable()
    .default(null),
  hidden_outdated_count_capped: z.boolean().default(false),
});
/** A search page whose request may have excluded the blueprint. */
const recordSearchPageSchema = recordSearchResponseSchema.extend({
  blueprint: blueprintWithAttributesSchema.optional(),
});
export const recordHierarchySchema = z.object({
  items: z.array(z.object({ id: uuidSchema, display: z.string() })),
  paths: z.array(z.array(z.object({ id: uuidSchema, display: z.string() }))),
  truncated: z.boolean(),
  multiple_parents: z.boolean(),
  cycle_detected: z.boolean(),
});
const recordAuditChangeSchema = z.object({
  audit_event_id: uuidSchema,
  occurred_at: z.string(),
  actor_user_id: uuidSchema.nullable(),
  actor_display_name: z.string().nullable(),
  actor_email: z.string().nullable(),
  actor_avatar_file_id: uuidSchema.nullable().default(null),
  executor_type: z.string(),
  agent_run_id: uuidSchema.nullable(),
  approval_decision: z.string().nullable(),
  approved_by_user_id: uuidSchema.nullable(),
  approved_by_display_name: z.string().nullable(),
  approved_by_avatar_file_id: uuidSchema.nullable().default(null),
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
const reusableRecordAttributeSchema = attributeSchema.extend({
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
const recordFormResponseSchema = z.object({
  can_write: z.boolean().default(false),
  record: recordSchema,
  blueprint: blueprintWithAttributesSchema,
  values: z.array(formAttributeValueSchema),
  reusable_attributes: z.array(reusableRecordAttributeSchema).default([]),
  reusable_values: z.array(formAttributeValueSchema).default([]),
  context: recordContextSchema,
});
const migrationIssueSchema = z.object({
  attribute_code: z.string().nullable(),
  // The kinds the API reports; an unknown one is shown with its message.
  kind: z.string(),
  message: z.string(),
});
export const recordMigrationPreviewSchema = z.object({
  source_updated_at: z.string().datetime({ offset: true }).optional(),
  migration_id: uuidSchema,
  source_version: z.number().int().positive(),
  target: blueprintWithAttributesSchema,
  values: z.array(formAttributeValueSchema),
  status: z.enum(['ready', 'needs_input', 'blocked']),
  issues: z.array(migrationIssueSchema),
});
const attributeFilterOperatorSchema = z.enum(attributeFilterOperators);
export const recordSearchFilterSchema = z
  .object({
    field: z.string().min(1),
    operator: attributeFilterOperatorSchema,
    value: z.union([z.string(), z.number().finite(), z.boolean()]),
  })
  .refine(
    (filter) =>
      filter.operator !== attributeFilterOperators.isSet ||
      typeof filter.value === 'boolean',
    {
      message: 'Presence filters require a boolean value',
      path: ['value'],
    },
  );
export const searchRecordsRequestSchema = z.object({
  blueprint: blueprintSelectorSchema,
  query: z.string(),
  filters: z.array(recordSearchFilterSchema),
  relationship_filters: z
    .array(
      z.object({
        field: z.string().min(1),
        selected_target_ids: z.array(uuidSchema).min(1),
      }),
    )
    .optional(),
  relationship_tree_facets: z
    .array(
      z.object({
        /** Omitted to facet the searched blueprint by its own hierarchy. */
        source_relationship_field: z.string().min(1).optional(),
        hierarchy_field: z.string().min(1).optional(),
        context_id: uuidSchema,
        selected_target_ids: z.array(uuidSchema).min(1),
      }),
    )
    .optional(),
  system_tags: z.array(z.string()).optional(),
  export_status: z
    .object({
      extension_id: z.string().min(1),
      connection: z.string().min(1).optional(),
      status: z.string().min(1),
    })
    .optional(),
  sort: z
    .object({
      field: z.string().min(1),
      direction: z.enum(['asc', 'desc']),
      context_code: z.string().min(1).optional(),
    })
    .optional(),
  data_quality: z.enum(dataQualityFilters).optional(),
  context_code: z.string().min(1).optional(),
  include_total: z.boolean().optional(),
  include_blueprint: z.boolean().optional(),
  include_previews: z.boolean().optional(),
  page: z.object({
    size: z.number().int().positive(),
    cursor: z.string().nullable(),
  }),
});
export const relationshipTreeFacetChildrenRequestSchema = z.object({
  blueprint: blueprintSelectorSchema,
  query: z.string().optional(),
  /** Omitted to list the blueprint's own tree through `hierarchy_field`. */
  source_relationship_field: z.string().min(1).optional(),
  hierarchy_field: z.string().min(1).optional(),
  context_id: uuidSchema,
  parent_id: uuidSchema.optional(),
  cursor: uuidSchema.nullable().optional(),
  selected_target_ids: z.array(uuidSchema).optional(),
  include_ids: z.array(uuidSchema).optional(),
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
  /** Root-first IDs down to each selected node, ending with the node. */
  selected_paths: z.array(z.array(uuidSchema)),
  next_cursor: uuidSchema.nullable(),
});
export const systemTagFacetRequestSchema = z.object({
  blueprint: blueprintSelectorSchema,
  prefix: z.string().optional(),
  limit: z.number().int().positive().optional(),
});
export const systemTagFacetResponseSchema = z.object({
  items: z.array(
    z.object({ tag: z.string(), count: z.number().int().nonnegative() }),
  ),
  /** More tags match than `items` lists. */
  truncated: z.boolean(),
});
const getBlueprintRevisionRequestSchema = z.object({
  id: uuidSchema,
  version: z.number().int().positive(),
});
const newFileAttributeValueSchema = attributeValueTargetSchema.extend({
  file_ids: z.array(uuidSchema).min(1),
});
export const createRecordRequestSchema = z.object({
  blueprint: blueprintSelectorSchema,
  values: z.array(newAttributeValueSchema),
  /** Staged uploads the create claims, one ordered list per attribute. */
  files: z.array(newFileAttributeValueSchema).optional(),
  system_tags: z.array(z.string()).optional(),
  system_metadata: jsonObjectSchema.optional(),
});
export const updateRecordRequestSchema = z.object({
  expected_updated_at: z.string().datetime({ offset: true }).optional(),
  values: z.array(newAttributeValueSchema),
  relationships: z.array(relationshipTargetsSchema),
  remove_values: z.array(attributeValueSelectorSchema).default([]),
  system_tags: z.array(z.string()).optional(),
  system_metadata: jsonObjectSchema.optional(),
});
export const migrateRecordRequestSchema = z.object({
  expected_updated_at: z.string().datetime({ offset: true }).optional(),
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
export type NewFileAttributeValue = z.infer<typeof newFileAttributeValueSchema>;
export type FormAttributeValue = z.infer<typeof formAttributeValueSchema>;
export type RelationshipTargets = z.infer<typeof relationshipTargetsSchema>;
export type RelationshipTreeFacetChildrenRequest = z.infer<
  typeof relationshipTreeFacetChildrenRequestSchema
>;
export type RelationshipTreeFacetChild = z.infer<
  typeof relationshipTreeFacetChildrenResponseSchema
>['items'][number];
export type AttricatRecord = z.infer<typeof recordSchema>;
export type RecordPublicationStatus = z.infer<
  typeof recordPublicationStatusSchema
>;
export type { CheckViolation } from '../../api/checkViolations';
export type RecordPublicationReadiness = z.infer<
  typeof recordPublicationReadinessSchema
>;
export type RecordAuditChange = z.infer<typeof recordAuditChangeSchema>;
export type RecordItem = z.infer<typeof recordItemSchema>;
export type SystemTagFacetRequest = z.infer<typeof systemTagFacetRequestSchema>;
export type SystemTagFacetResponse = z.infer<
  typeof systemTagFacetResponseSchema
>;
export type SystemTagCount = z.infer<
  typeof systemTagFacetResponseSchema
>['items'][number];
export type RecordSearchFilter = z.infer<typeof recordSearchFilterSchema>;
export type RecordSearchResponse = z.infer<typeof recordSearchResponseSchema>;
export type RecordFormResponse = z.infer<typeof recordFormResponseSchema>;
export type ResolvedRecordPreview = z.infer<typeof resolvedRecordPreviewSchema>;
export type RecordMigrationPreview = z.infer<
  typeof recordMigrationPreviewSchema
>;

export {
  recordFormResponseSchema,
  recordSearchResponseSchema,
  recordSearchPageSchema,
  blueprintSelectorSchema,
  getBlueprintRevisionRequestSchema,
  resolvedRecordPreviewSchema,
};

export const statusTransitionAccessSchema = z.object({
  attribute_code: z.string(),
  from: z.string().nullable(),
  to: z.string().nullable(),
  code: z.string().nullable(),
  allowed: z.boolean(),
  denial_code: z.enum(statusTransitionDenialCodes).nullable(),
  denial_reason: z.string().nullable(),
  /**
   * Allowed only because the caller holds `records.unlock`: they made the
   * earlier step that separation of duties would require someone else for.
   */
  waives_separation: z.boolean().optional(),
  /** Unmet transition conditions and enforcing rules, as check violations. */
  unmet: z.array(checkViolationSchema).optional(),
});
export type StatusTransitionAccess = z.infer<
  typeof statusTransitionAccessSchema
>;

export const recordApprovalSchema = z.object({
  id: z.string(),
  attribute_code: z.string(),
  context_code: z.string(),
  status: z.string(),
  covers_all: z.boolean(),
  covered_attributes: z.array(z.string()),
  content_digest: z.string(),
  approved_by_user_id: uuidSchema.nullable(),
  approved_by: z.string().nullable(),
  approved_at: z.string(),
  ended_at: z.string().nullable(),
  end_reason: z.enum(approvalEndReasons).nullable(),
  void_status: z.string().nullable(),
  /** The status change that entered the approved status; `null` when it was not recorded. */
  cause: z
    .object({
      from: z.string().nullable(),
      transition_code: z.string().nullable(),
      kind: z.enum(approvalCauseKinds),
      operation: z.enum(transitionOperations).nullable(),
      channel: z.enum(transitionChannels).nullable(),
      separation_waived: z.boolean(),
    })
    .nullable(),
});
export type ApprovalCause = NonNullable<RecordApproval['cause']>;
export type RecordApproval = z.infer<typeof recordApprovalSchema>;

export const retentionHoldSchema = z.object({
  id: z.string(),
  file_id: z.string(),
  source: z.enum(retentionHoldSources),
  attribute_code: z.string().nullable(),
  status: z.string().nullable(),
  reason: z.string().nullable(),
  held_until: z.string(),
  created_at: z.string(),
  released_at: z.string().nullable(),
  active: z.boolean(),
});
export type RetentionHold = z.infer<typeof retentionHoldSchema>;

/** One newest-first page of a record's approvals or retention holds. */
export const recordControlPageSchema = <T extends z.ZodType>(item: T) =>
  z.object({ items: z.array(item), has_more: z.boolean() });

/** Continues a record control list before its last item. */

/** Unwraps an `{ items: [...] }` collection response. */
export const itemsResponseSchema = <T extends z.ZodType>(item: T) =>
  z.object({ items: z.array(item) }).transform(({ items }) => items);

/** One page of a record's change history. */
export const recordChangesPageSchema = z.object({
  items: z.array(recordAuditChangeSchema),
  next_offset: z.number().int().nonnegative().nullable(),
});

/** Publication statuses of several records, keyed by record. */
export const recordsPublicationsResponseSchema = z.object({
  items: z.array(
    z.object({
      record_id: uuidSchema,
      publications: z.array(recordPublicationStatusSchema),
    }),
  ),
});

/** The `details` of a `record_schema_mismatch` rejection. */
export const schemaMismatchDetailsSchema = z.object({
  instance_path: z.string().default(''),
});
