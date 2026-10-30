import i18n from '../../i18n';
import principalContract from '../../../../../contracts/principal-attribute-v1.schema.json';
import statusContract from '../../../../../contracts/status-control-v1.schema.json';
import { definitionKinds, schemaAnnotations } from './constants';
import {
  embeddedJsonKinds,
  type EmbeddedJsonKind,
} from './definitionEmbeddedReferences';
import {
  definitionSchemas,
  parseSchemaNode,
  type SchemaNode,
} from './definitionSchema';

const refPrefix = '#/$defs/';
const definitions = {
  jsonSchema: 'JsonSchema',
  predicate: 'Predicate',
  principal: 'PrincipalAttribute',
  status: 'StatusControl',
} as const;
const ref = (name: string, description?: string): SchemaNode => ({
  $ref: `${refPrefix}${name}`,
  description,
});

/** The JSON Schema keywords the editor explains. */
const keywords = [
  '$schema',
  'title',
  'description',
  'type',
  'enum',
  'const',
  'default',
  'format',
  'pattern',
  'minLength',
  'maxLength',
  'minimum',
  'maximum',
  'exclusiveMinimum',
  'exclusiveMaximum',
  'multipleOf',
  'items',
  'minItems',
  'maxItems',
  'uniqueItems',
  'properties',
  'required',
  'additionalProperties',
  'dependentRequired',
  'dependentSchemas',
  'allOf',
  'anyOf',
  'oneOf',
  'not',
  'if',
  'then',
  'else',
] as const;
type Keyword = (typeof keywords)[number];

const jsonTypes = [
  'string',
  'number',
  'integer',
  'boolean',
  'object',
  'array',
  'null',
];

/** Keywords whose value is itself a schema, or a list or map of them. */
const keywordShapes: Partial<Record<Keyword, SchemaNode>> = {
  additionalProperties: ref(definitions.jsonSchema),
  allOf: { items: ref(definitions.jsonSchema), type: 'array' },
  anyOf: { items: ref(definitions.jsonSchema), type: 'array' },
  dependentRequired: {
    additionalProperties: { items: { type: 'string' }, type: 'array' },
    type: 'object',
  },
  dependentSchemas: {
    additionalProperties: ref(definitions.jsonSchema),
    type: 'object',
  },
  else: ref(definitions.jsonSchema),
  enum: { type: 'array' },
  if: ref(definitions.jsonSchema),
  items: ref(definitions.jsonSchema),
  not: ref(definitions.jsonSchema),
  oneOf: { items: ref(definitions.jsonSchema), type: 'array' },
  properties: {
    additionalProperties: ref(definitions.jsonSchema),
    type: 'object',
  },
  required: { items: { type: 'string' }, type: 'array' },
  then: ref(definitions.jsonSchema),
  type: { enum: jsonTypes },
};

const keywordProperties = () =>
  Object.fromEntries(
    keywords.map((keyword) => [
      keyword,
      {
        ...keywordShapes[keyword],
        description: i18n.t(
          `definitionEditor.jsonSchema.keywords.${keyword.replace('$', '')}`,
        ),
      },
    ]),
  );

/** Status conditions use the predicates that rules and record checks use. */
const statusControl = (): SchemaNode => {
  const status = structuredClone(parseSchemaNode(statusContract));
  const condition =
    status.properties?.transitions?.items?.properties?.conditions?.items;
  if (condition?.properties)
    condition.properties.predicate = ref(
      definitions.predicate,
      condition.properties.predicate?.description,
    );
  return status;
};

const recordChecks = (): SchemaNode => ({
  description: i18n.t('definitionEditor.jsonSchema.checks.description'),
  items: {
    properties: {
      code: {
        description: i18n.t('definitionEditor.jsonSchema.checks.code'),
        type: 'string',
      },
      message: {
        description: i18n.t('definitionEditor.jsonSchema.checks.message'),
        type: 'string',
      },
      predicate: ref(
        definitions.predicate,
        i18n.t('definitionEditor.jsonSchema.checks.predicate'),
      ),
    },
    required: ['code', 'predicate'],
    type: 'object',
  },
  type: 'array',
});

/**
 * Contracts for the JSON in `record_schema` and `value_schema`, for hovers:
 * JSON Schema keywords plus the Attricat annotations each allows, with
 * predicates from the rule contract.
 */
export const embeddedJsonSchemas = (): Record<EmbeddedJsonKind, SchemaNode> => {
  const keywordsByName = keywordProperties();
  const $defs: Record<string, SchemaNode> = {
    ...definitionSchemas[definitionKinds.rule].$defs,
    [definitions.jsonSchema]: { properties: keywordsByName, type: 'object' },
    [definitions.principal]: parseSchemaNode(principalContract),
    [definitions.status]: statusControl(),
  };
  const annotation = (name: string, key: string) =>
    ref(name, i18n.t(`definitionEditor.jsonSchema.${key}`));
  return {
    [embeddedJsonKinds.recordSchema]: {
      $defs,
      properties: {
        ...keywordsByName,
        [schemaAnnotations.checks]: recordChecks(),
      },
      type: 'object',
    },
    [embeddedJsonKinds.valueSchema]: {
      $defs,
      properties: {
        ...keywordsByName,
        [schemaAnnotations.enumLabels]: {
          additionalProperties: { type: 'string' },
          description: i18n.t('definitionEditor.jsonSchema.enumLabels'),
          type: 'object',
        },
        [schemaAnnotations.patternHint]: {
          description: i18n.t('definitionEditor.jsonSchema.patternHint'),
          type: 'string',
        },
        [schemaAnnotations.principal]: annotation(
          definitions.principal,
          'principal',
        ),
        [schemaAnnotations.status]: annotation(definitions.status, 'status'),
      },
      type: 'object',
    },
  };
};
