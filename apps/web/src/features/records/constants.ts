import { checkViolationErrorCodes } from '../../api/checkViolations';
import type { ApiErrorCode } from '../../api/errors';

/** Largest integer the API accepts for component protocol versions (Rust u32). */
export const MAX_COMPONENT_PROTOCOL_VERSION = 4_294_967_295;

/** Smart fill request limits enforced by the API. */
export const SMART_FILL_MAX_CONTENT_LENGTH = 32_768;
export const SMART_FILL_MAX_ATTACHMENTS = 16;

/** Why a retention hold keeps a record: its status, or an explicit hold. */
export const retentionHoldSources = {
  status: 'status',
  explicit: 'explicit',
} as const;

export const RECORD_SEARCH_PAGE_SIZE = 25;
/**
 * System tag of solution-pack sample records. Records show it as the sample
 * badge instead of a tag chip.
 */
export const SAMPLE_SYSTEM_TAG = 'attricat.sample';
/** System tag Core keeps on records with at least one open rule finding. */
export const NEEDS_ATTENTION_SYSTEM_TAG = 'attricat.needs_attention';
/**
 * Longest system tag, in UTF-8 bytes, as the API's `MAX_TAG_LENGTH`; the
 * facet refuses a longer prefix.
 */
export const MAX_SYSTEM_TAG_BYTES = 128;
/** Most tags one system tag facet request lists; the API caps it at 100. */
export const SYSTEM_TAG_FACET_LIMIT = 50;
/**
 * Operators a record search filter compares an attribute value with; `isSet`
 * distinguishes an absent value from a present one.
 */
export const attributeFilterOperators = {
  eq: 'eq',
  contains: 'contains',
  startsWith: 'starts_with',
  gt: 'gt',
  gte: 'gte',
  lt: 'lt',
  lte: 'lte',
  isSet: 'is_set',
} as const;
/** Most record IDs one label lookup may name; the API enforces the same. */
export const RECORD_LABEL_BATCH_SIZE = 100;
export const RECORD_CHANGES_PAGE_SIZE = 25;

/** Sections shown in tabs below a record's values. */
export const recordPreviewTabs = {
  dataQuality: 'data-quality',
  comments: 'comments',
  conversations: 'conversations',
  recordControl: 'record-control',
  systemAnnotations: 'system-annotations',
  exports: 'exports',
} as const;
export type RecordPreviewTabKey =
  (typeof recordPreviewTabs)[keyof typeof recordPreviewTabs];

export const RECORD_EXTENSION_DRAWER_ID = 'record-extension-contributions';
export const RECORD_DRAWER_WIDTH = 480;

/** Separators for relationship target IDs stored in a single form field. */
export const RELATIONSHIP_ID_SEPARATOR = ',';
export const RELATIONSHIP_ID_JOINER = ', ';

export const attributeCardinalities = {
  one: 'one',
  many: 'many',
} as const;

/** How a self-referencing relationship may link records to each other. */
export const attributeHierarchies = {
  acyclic: 'acyclic',
  tree: 'tree',
} as const;

/** Record check predicates that walk a relationship. */
export const relationshipPredicateTypes = {
  acyclic: 'acyclic',
  linked: 'linked',
  referencedBy: 'referenced_by',
} as const;

/** The record schema keyword holding declarative record checks. */
export const recordSchemaChecksKeyword = 'x-attricat-checks';

export const attributeContextEditability = {
  all: 'all',
  default: 'default',
} as const;

/** When a generated attribute's numbering restarts at its start. */
export const generatedSequenceResets = {
  never: 'never',
  yearly: 'yearly',
  monthly: 'monthly',
  daily: 'daily',
} as const;

export const attributeContextFallbacks = {
  default: 'default',
  none: 'none',
} as const;

export const migrationIssueKinds = {
  removed: 'removed',
  valueTypeChanged: 'value_type_changed',
  relationshipTargetChanged: 'relationship_target_changed',
  relationshipCardinalityChanged: 'relationship_cardinality_changed',
  relationshipCardinalityConflict: 'relationship_cardinality_conflict',
  fileContractChanged: 'file_contract_changed',
  contextNotEditable: 'context_not_editable',
  attributeSchemaMismatch: 'attribute_schema_mismatch',
  missingRequired: 'missing_required',
} as const;

export const publicationStatuses = {
  notPublished: 'not_published',
  published: 'published',
} as const;

/** Whether a search's results share one blueprint version. */
export const resultVersionScopeKinds = {
  empty: 'empty',
  single: 'single',
  multiple: 'multiple',
} as const;

/** Why an approval stopped applying. */
export const approvalEndReasons = {
  contentChanged: 'content_changed',
  superseded: 'superseded',
} as const;

/** Status coverage that locks or approves every other attribute. */
export const statusCoverageAll = 'all';

/** Separates a reusable attribute namespace from its code. */
export const REUSABLE_ATTRIBUTE_NAMESPACE_SEPARATOR = ':';

export const reusableSelectionTypes = {
  attribute: 'attribute',
  group: 'group',
} as const;

export type ReusableSelectionType =
  (typeof reusableSelectionTypes)[keyof typeof reusableSelectionTypes];

export const booleanFieldValues = {
  true: 'true',
  false: 'false',
} as const;

/** Example formats for scalar types whose built-in input is plain text. */
export const scalarValuePlaceholders: Partial<Record<string, string>> = {
  datetime: '2026-08-19T12:00:00Z',
  time: '09:30:00 America/New_York',
  json: '{\n  "key": "value"\n}',
};
export const JSON_EDITOR_MIN_ROWS = 4;

/** Number of contexts shown as tabs before the remainder moves to a menu. */
export const CONTEXT_TAB_LIMIT = 5;
export const CONTEXT_TAB_MIN_HEIGHT = 32;
export const CONTEXT_MENU_WIDTH = 180;

export const RELATIONSHIP_PICKER_ACTION_MIN_WIDTH = 300;

/** Blueprint version assumed by the extension drawer before data loads. */
export const FALLBACK_BLUEPRINT_VERSION = 1;

/** Why the server refuses a status transition (`denial_code`). */
export const statusTransitionDenialCodes = {
  forbidden: 'status_transition_forbidden',
  separationOfDuties: 'status_separation_of_duties',
  /** Also the API error code whose details list the unmet conditions. */
  conditionsUnmet: checkViolationErrorCodes.transitionConditionsUnmet,
} as const satisfies Record<string, ApiErrorCode>;
/** API error code for a write based on an outdated record version. */
export const STALE_RECORD_ERROR_CODE = 'stale_record' satisfies ApiErrorCode;
/** API error code for a write that leaves the record outside its JSON Schema. */
export const RECORD_SCHEMA_MISMATCH_ERROR_CODE =
  'record_schema_mismatch' satisfies ApiErrorCode;

/** Matches editors of fields a migration asks the user to review. */
export const migrationReviewSelector = '[data-migration-review]';

/** How long a field shows that it was saved, and its save can be undone. */
export const RECENT_SAVE_DURATION_MS = 6000;

/** Where one inline field's change stands. */
export const fieldSaveStates = {
  saving: 'saving',
  saved: 'saved',
  unsaved: 'unsaved',
} as const;
export type FieldSaveState =
  (typeof fieldSaveStates)[keyof typeof fieldSaveStates];

/** How a save that conflicts with another editor's change is settled. */
export const conflictResolutions = {
  keepMine: 'keepMine',
  useTheirs: 'useTheirs',
} as const;
export type ConflictResolution =
  (typeof conflictResolutions)[keyof typeof conflictResolutions];
