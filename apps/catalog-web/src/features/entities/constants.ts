import { checkViolationErrorCodes } from '../../api/checkViolations';

/** Largest integer the API accepts for component protocol versions (Rust u32). */
export const MAX_COMPONENT_PROTOCOL_VERSION = 4_294_967_295;

/** Smart fill request limits enforced by the API. */
export const SMART_FILL_MAX_CONTENT_LENGTH = 32_768;
export const SMART_FILL_MAX_ATTACHMENTS = 16;

export const ENTITY_SEARCH_PAGE_SIZE = 25;
/** Most entity IDs one label lookup may name; the API enforces the same. */
export const ENTITY_LABEL_BATCH_SIZE = 100;
export const ENTITY_CHANGES_PAGE_SIZE = 25;

/** Query parameters used when fetching a lightweight entity preview. */
export const ENTITY_PREVIEW_QUERY = {
  relationshipDepth: 0,
  relationshipLimit: 1,
} as const;

export const EDIT_ENTITY_FORM_ID = 'edit-entity-form';
export const ENTITY_EXTENSION_DRAWER_ID = 'entity-extension-contributions';
export const ENTITY_DRAWER_WIDTH = 480;

/** Separators for relationship target IDs stored in a single form field. */
export const RELATIONSHIP_ID_SEPARATOR = ',';
export const RELATIONSHIP_ID_JOINER = ', ';

export const attributeCardinalities = {
  one: 'one',
  many: 'many',
} as const;

export const attributeContextEditability = {
  all: 'all',
  default: 'default',
} as const;

export const attributeContextFallbacks = {
  default: 'default',
  none: 'none',
} as const;

export const migrationIssueKinds = {
  removed: 'removed',
  relationshipTargetChanged: 'relationship_target_changed',
  missingRequired: 'missing_required',
} as const;

export const publicationStatuses = {
  notPublished: 'not_published',
  published: 'published',
} as const;

export const AGENT_EXECUTOR_TYPE = 'agent';

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

/** Distance of the floating smart fill button from the viewport edges. */
export const SMART_FILL_BUTTON_OFFSET = 24;

export const CONVERSATION_TITLE_POLL_INTERVAL = 3_000;
/** Number of entity ID characters included in a new conversation title. */
export const CONVERSATION_ENTITY_ID_PREFIX_LENGTH = 8;

/** Version of the extension context passed to entity header actions. */
export const ENTITY_HEADER_CONTEXT_VERSION = 1;
/** Blueprint version assumed by the extension drawer before data loads. */
export const FALLBACK_BLUEPRINT_VERSION = 1;

/** Why the server refuses a status transition (`denial_code`). */
export const statusTransitionDenialCodes = {
  forbidden: 'status_transition_forbidden',
  separationOfDuties: 'status_separation_of_duties',
  /** Also the API error code whose details list the unmet conditions. */
  conditionsUnmet: checkViolationErrorCodes.transitionConditionsUnmet,
} as const;
export type StatusTransitionDenialCode =
  (typeof statusTransitionDenialCodes)[keyof typeof statusTransitionDenialCodes];
