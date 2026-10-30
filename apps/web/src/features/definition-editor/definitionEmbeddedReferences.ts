import { parseTree, type Node as JsonNode } from 'jsonc-parser';
import type { TextRange, YamlEntry } from '../../components/yamlOutline';
import { attributesKey, schemaAnnotations } from './constants';
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

/** JSON Schema keywords at a record schema's root that name attributes. */
const schemaKeywords = {
  dependentRequired: 'dependentRequired',
  dependentSchemas: 'dependentSchemas',
  properties: 'properties',
  required: 'required',
} as const;

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
  ...items(member(root, schemaAnnotations.checks)).flatMap((check) =>
    predicateAttributes(member(check, predicateKeys.predicate), true),
  ),
];

/** Status transition conditions are about the record that holds the value. */
const valueSchemaAttributes = (root: JsonNode) =>
  items(member(member(root, schemaAnnotations.status), transitionsKey)).flatMap(
    (transition) =>
      items(member(transition, conditionsKey)).flatMap((condition) =>
        predicateAttributes(member(condition, predicateKeys.predicate), true),
      ),
  );

/** Keys whose string values are JSON Schemas. */
export const embeddedJsonKinds = {
  recordSchema: 'record_schema',
  valueSchema: 'value_schema',
} as const;
export type EmbeddedJsonKind =
  (typeof embeddedJsonKinds)[keyof typeof embeddedJsonKinds];

const embeddedJsonKind = ({
  path,
}: YamlEntry): EmbeddedJsonKind | undefined => {
  if (path.length === 1 && path[0] === embeddedJsonKinds.recordSchema)
    return embeddedJsonKinds.recordSchema;
  // An attribute of a blueprint, or a reusable attribute itself.
  const attribute =
    path.length === 1 || (path.length === 3 && path[0] === attributesKey);
  return attribute && path.at(-1) === embeddedJsonKinds.valueSchema
    ? embeddedJsonKinds.valueSchema
    : undefined;
};

/**
 * The JSON that a `record_schema` or `value_schema` entry holds, parsed
 * tolerantly so JSON being typed still yields what it can. Node offsets are
 * relative to `start` in the definition text.
 */
export const embeddedJson = (text: string, entry: YamlEntry) => {
  const { valueRange } = entry;
  const kind = embeddedJsonKind(entry);
  if (!kind || typeof entry.value !== 'string' || !valueRange) return undefined;
  // A double-quoted scalar escapes the JSON's quotes, so its text is not the
  // JSON the offsets would point into.
  if (text[valueRange.start] === '"') return undefined;
  // Block scalars start with their indicator; the JSON starts at its brace.
  const start = text.indexOf('{', valueRange.start);
  if (start < 0 || start >= valueRange.end) return undefined;
  const root = parseTree(text.slice(start, valueRange.end), [], {
    allowTrailingComma: true,
  });
  return root && { kind, root, start };
};

/**
 * Attributes that a blueprint's `record_schema` or an attribute's
 * `value_schema` names in its JSON: schema keywords, record checks and status
 * transition conditions.
 */
export const embeddedReferences = (
  text: string,
  entry: YamlEntry,
): EmbeddedReference[] => {
  const json = embeddedJson(text, entry);
  if (!json) return [];
  const walk =
    json.kind === embeddedJsonKinds.recordSchema
      ? recordSchemaAttributes
      : valueSchemaAttributes;
  return walk(json.root).map(({ node, reference }) => ({
    range: {
      end: json.start + node.offset + node.length,
      start: json.start + node.offset,
    },
    reference,
    value: String(node.value),
  }));
};
