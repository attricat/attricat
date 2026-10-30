import { parseTree, type Node as JsonNode } from 'jsonc-parser';
import type { TextRange, YamlEntry } from '../../components/yamlOutline';
import { attributesKey } from './constants';
import {
  definitionReferenceKinds,
  type DefinitionReferenceKind,
} from './definitionSchema';

/** An attribute named inside JSON that a definition embeds as a string. */
export type EmbeddedReference = {
  /** The JSON string, including its quotes. */
  range: TextRange;
  reference: DefinitionReferenceKind;
  value: string;
};

/** Blueprint keys whose string values are JSON Schemas naming attributes. */
const recordSchemaKey = 'record_schema';
const valueSchemaKey = 'value_schema';

/** JSON Schema keywords at a record schema's root that name attributes. */
const schemaKeywords = {
  dependentRequired: 'dependentRequired',
  dependentSchemas: 'dependentSchemas',
  properties: 'properties',
  required: 'required',
} as const;

const checksKey = 'x-attricat-checks';
const statusKey = 'x-attricat-status';
const transitionsKey = 'transitions';
const conditionsKey = 'conditions';

/** Predicate fields; see `docs/json-schema-validation.md#predicates`. */
const predicateKeys = {
  attributeCode: 'attribute_code',
  attributeCodes: 'attribute_codes',
  otherAttributeCode: 'other_attribute_code',
  predicate: 'predicate',
  predicates: 'predicates',
  relationshipCode: 'relationship_code',
  subjectAttributeCode: 'subject_attribute_code',
  type: 'type',
} as const;
/** Predicates whose nested `predicate` is about other records. */
const otherRecordPredicates = new Set(['linked', 'referenced_by']);
/** The predicate whose `relationship_code` belongs to another blueprint. */
const referencedByPredicate = 'referenced_by';

const member = (node: JsonNode | undefined, key: string) =>
  node?.type === 'object'
    ? node.children?.find((property) => property.children?.[0]?.value === key)
        ?.children?.[1]
    : undefined;

const items = (node: JsonNode | undefined) =>
  node?.type === 'array' ? (node.children ?? []) : [];

const keys = (node: JsonNode | undefined) =>
  node?.type === 'object'
    ? (node.children ?? []).flatMap((property) => property.children?.[0] ?? [])
    : [];

const strings = (nodes: (JsonNode | undefined)[]) =>
  nodes.filter(
    (node): node is JsonNode =>
      node?.type === 'string' && typeof node.value === 'string',
  );

/**
 * Attributes a predicate names. `ownRecord` is false inside `linked` and
 * `referenced_by`, where plain attribute codes belong to the other records.
 */
const predicateAttributes = (
  predicate: JsonNode | undefined,
  ownRecord: boolean,
): { node: JsonNode; reference: DefinitionReferenceKind }[] => {
  if (predicate?.type !== 'object') return [];
  const type = member(predicate, predicateKeys.type)?.value;
  const attribute = definitionReferenceKinds.attribute;
  const own = ownRecord
    ? [
        member(predicate, predicateKeys.attributeCode),
        member(predicate, predicateKeys.otherAttributeCode),
        ...items(member(predicate, predicateKeys.attributeCodes)),
      ]
    : [];
  return [
    ...strings([
      ...own,
      member(predicate, predicateKeys.subjectAttributeCode),
    ]).map((node) => ({ node, reference: attribute })),
    ...strings([
      ownRecord && type !== referencedByPredicate
        ? member(predicate, predicateKeys.relationshipCode)
        : undefined,
    ]).map((node) => ({
      node,
      reference: definitionReferenceKinds.relationshipAttribute,
    })),
    ...predicateAttributes(
      member(predicate, predicateKeys.predicate),
      ownRecord && !otherRecordPredicates.has(String(type)),
    ),
    ...items(member(predicate, predicateKeys.predicates)).flatMap((nested) =>
      predicateAttributes(nested, ownRecord),
    ),
  ];
};

const recordSchemaAttributes = (root: JsonNode) => [
  ...strings([
    ...items(member(root, schemaKeywords.required)),
    ...keys(member(root, schemaKeywords.properties)),
    ...keys(member(root, schemaKeywords.dependentSchemas)),
    ...keys(member(root, schemaKeywords.dependentRequired)),
    ...(member(root, schemaKeywords.dependentRequired)?.children ?? []).flatMap(
      (property) => items(property.children?.[1]),
    ),
  ]).map((node) => ({ node, reference: definitionReferenceKinds.attribute })),
  ...items(member(root, checksKey)).flatMap((check) =>
    predicateAttributes(member(check, predicateKeys.predicate), true),
  ),
];

/** Status transition conditions are about the record that holds the value. */
const valueSchemaAttributes = (root: JsonNode) =>
  items(member(member(root, statusKey), transitionsKey)).flatMap((transition) =>
    items(member(transition, conditionsKey)).flatMap((condition) =>
      predicateAttributes(member(condition, predicateKeys.predicate), true),
    ),
  );

const isRecordSchema = ({ path }: YamlEntry) =>
  path.length === 1 && path[0] === recordSchemaKey;
const isValueSchema = ({ path }: YamlEntry) =>
  path.length === 3 && path[0] === attributesKey && path[2] === valueSchemaKey;

/**
 * Attributes that a blueprint's `record_schema` or an attribute's
 * `value_schema` names in its JSON: schema keywords, record checks and status
 * transition conditions. JSON that is still being typed yields what parses.
 */
export const embeddedReferences = (
  text: string,
  entry: YamlEntry,
): EmbeddedReference[] => {
  const { valueRange } = entry;
  if (typeof entry.value !== 'string' || !valueRange) return [];
  const walk = isRecordSchema(entry)
    ? recordSchemaAttributes
    : isValueSchema(entry)
      ? valueSchemaAttributes
      : undefined;
  // A double-quoted scalar escapes the JSON's quotes, so its text is not the
  // JSON the offsets would point into.
  if (!walk || text[valueRange.start] === '"') return [];
  // Block scalars start with their indicator; the JSON starts at its brace.
  const start = text.indexOf('{', valueRange.start);
  if (start < 0 || start >= valueRange.end) return [];
  const root = parseTree(text.slice(start, valueRange.end), [], {
    allowTrailingComma: true,
  });
  if (!root) return [];
  return walk(root).map(({ node, reference }) => ({
    range: {
      end: start + node.offset + node.length,
      start: start + node.offset,
    },
    reference,
    value: String(node.value),
  }));
};
