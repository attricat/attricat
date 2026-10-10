export const definitionKinds = {
  blueprint: 'blueprint',
  reusableAttribute: 'reusableAttribute',
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

/** Tag field of the contracts' internally tagged unions. */
export const discriminatorKey = 'type';
export const attributesKey = 'attributes';
export const includesKey = 'includes';
export const valueTypeKey = 'value_type';
export const relationshipValueType = 'relationship';

export const attributePathSeparator = '.';
/** Table columns follow at most three relationship hops. */
export const maxAttributePathHops = 3;

export const diagnosticsOwner = 'attricat-definition';
export const diagnosticsDelayMs = 250;
/** Completion lookups reuse cached workspace data for this long. */
export const referenceStaleTimeMs = 60_000;
