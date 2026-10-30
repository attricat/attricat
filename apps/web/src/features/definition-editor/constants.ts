export const definitionKinds = {
  blueprint: 'blueprint',
  reusableAttribute: 'reusableAttribute',
  rule: 'rule',
  workflow: 'workflow',
} as const;
export type DefinitionKind =
  (typeof definitionKinds)[keyof typeof definitionKinds];

/** Editor hints the Rust contracts add with `#[schemars(extend(...))]`. */
export const schemaExtensions = {
  keySuggestions: 'x-attricat-key-suggestions',
  reference: 'x-attricat-reference',
  suggestions: 'x-attricat-suggestions',
  valueTypes: 'x-attricat-value-types',
} as const;

/** Attricat keywords inside the JSON Schemas a definition embeds. */
export const schemaAnnotations = {
  checks: 'x-attricat-checks',
  enumLabels: 'x-attricat-enum-labels',
  patternHint: 'x-attricat-pattern-hint',
  principal: 'x-attricat-principal',
  status: 'x-attricat-status',
} as const;

/** Tag field of the contracts' internally tagged unions. */
export const discriminatorKey = 'type';
export const attributesKey = 'attributes';
export const includesKey = 'includes';
export const viewsKey = 'views';
/** Keys of view blocks and columns whose values name an attribute. */
export const fieldKey = 'field';
export const fieldsKey = 'fields';
export const labelKey = 'label';
export const nameKey = 'name';
export const valueTypeKey = 'value_type';
export const codeKey = 'code';
export const fromKey = 'from';
export const targetBlueprintKey = 'target_blueprint';
export const aliasKey = 'alias';
export const versionKey = 'version';

/** The contracts' pattern for codes, which has its own message. */
export const codePattern = '^[A-Za-z0-9_-]+$';

export const attributePathSeparator = '.';
/** Completion sort prefixes: required keys and exact matches come first. */
export const requiredSortPrefix = '0';
export const optionalSortPrefix = '1';
/** Table columns follow at most three relationship hops. */
export const maxAttributePathHops = 3;

export const diagnosticsOwner = 'attricat-definition';
export const diagnosticsDelayMs = 250;
export const serverDiagnosticsOwner = 'attricat-definition-server';
/** Warnings for references to things the workspace does not have. */
export const referenceDiagnosticsOwner = 'attricat-definition-references';
/** Server checks wait for a longer pause than local diagnostics. */
export const checkDelayMs = 600;
/** How long a place shown from the preview stays highlighted. */
export const revealHighlightMs = 1500;
export const definitionEditorHeight = 'calc(100vh - 280px)';
/** Completion lookups reuse cached workspace data for this long. */
export const referenceStaleTimeMs = 60_000;

/**
 * Pages that reference values open with Ctrl+click, in a new tab so the
 * editor keeps its unsaved changes.
 */
export const referencePages = {
  blueprint: (id: string) => `/manage/blueprints/${encodeURIComponent(id)}`,
  contexts: '/manage/contexts',
  roles: '/manage/workspace/roles',
} as const;
