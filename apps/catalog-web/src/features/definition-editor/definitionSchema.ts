import { z } from 'zod';
import blueprintContract from '../../../../../contracts/blueprint-definition-v1.schema.json';
import reusableAttributeContract from '../../../../../contracts/reusable-attribute-definition-v1.schema.json';
import {
  definitionKinds,
  discriminatorKey,
  schemaExtensions,
  type DefinitionKind,
} from './constants';
import type { TomlPath } from './tomlOutline';

/**
 * The subset of JSON Schema that the Rust definition types generate. Unused
 * keywords are kept so Ajv validates against the full contract.
 */
export type SchemaNode = {
  $ref?: string;
  $defs?: Record<string, SchemaNode>;
  additionalProperties?: boolean | SchemaNode;
  anyOf?: SchemaNode[];
  const?: unknown;
  default?: unknown;
  description?: string;
  enum?: unknown[];
  items?: SchemaNode;
  oneOf?: SchemaNode[];
  properties?: Record<string, SchemaNode>;
  required?: string[];
  type?: string | string[];
  [schemaExtensions.keySuggestions]?: string[];
  [schemaExtensions.reference]?: DefinitionReferenceKind;
  [schemaExtensions.suggestions]?: string[];
  [schemaExtensions.valueTypes]?: string[];
};

const referenceKinds = [
  'attribute',
  'attribute_path',
  'blueprint',
  'context',
  'include_attribute',
  'mixin',
  'mixin_version',
  'relationship_attribute',
  'role',
  'view_component',
] as const;
export type DefinitionReferenceKind = (typeof referenceKinds)[number];

const schemaNodeSchema: z.ZodType<SchemaNode> = z.lazy(() =>
  z.looseObject({
    $ref: z.string().optional(),
    $defs: z.record(z.string(), schemaNodeSchema).optional(),
    additionalProperties: z.union([z.boolean(), schemaNodeSchema]).optional(),
    anyOf: z.array(schemaNodeSchema).optional(),
    const: z.unknown().optional(),
    default: z.unknown().optional(),
    description: z.string().optional(),
    enum: z.array(z.unknown()).optional(),
    items: schemaNodeSchema.optional(),
    oneOf: z.array(schemaNodeSchema).optional(),
    properties: z.record(z.string(), schemaNodeSchema).optional(),
    required: z.array(z.string()).optional(),
    type: z.union([z.string(), z.array(z.string())]).optional(),
    [schemaExtensions.keySuggestions]: z.array(z.string()).optional(),
    [schemaExtensions.reference]: z.enum(referenceKinds).optional(),
    [schemaExtensions.suggestions]: z.array(z.string()).optional(),
    [schemaExtensions.valueTypes]: z.array(z.string()).optional(),
  }),
);

export const definitionSchemas: Record<DefinitionKind, SchemaNode> = {
  [definitionKinds.blueprint]: schemaNodeSchema.parse(blueprintContract),
  [definitionKinds.reusableAttribute]: schemaNodeSchema.parse(
    reusableAttributeContract,
  ),
};

const refPrefix = '#/$defs/';
const nullType = 'null';

const isNullSchema = (node: SchemaNode) => node.type === nullType;

/**
 * Follows `$ref`s and unwraps `Option<T>` (`anyOf: [T, null]`), keeping the
 * field-level description and editor hints from the wrapper.
 */
export const unwrapSchema = (
  root: SchemaNode,
  node: SchemaNode,
): SchemaNode => {
  let current = node;
  const overrides: SchemaNode = {};
  for (;;) {
    const { $ref, anyOf, ...rest } = current;
    Object.assign(overrides, {
      ...Object.fromEntries(
        Object.entries(rest).filter(([, value]) => value !== undefined),
      ),
      ...overrides,
    });
    if ($ref?.startsWith(refPrefix)) {
      const target = root.$defs?.[$ref.slice(refPrefix.length)];
      if (!target) return overrides;
      current = target;
      continue;
    }
    const present = anyOf?.filter((option) => !isNullSchema(option));
    if (present?.length === 1) {
      current = present[0];
      continue;
    }
    return anyOf ? { ...overrides, anyOf } : overrides;
  }
};

export const primaryType = (node: SchemaNode) =>
  (Array.isArray(node.type) ? node.type : [node.type]).find(
    (type) => type !== undefined && type !== nullType,
  );

/** Returns the tagged variants when every `oneOf` branch has a constant `type`. */
export const taggedVariants = (root: SchemaNode, node: SchemaNode) => {
  const variants = node.oneOf?.map((variant) => unwrapSchema(root, variant));
  if (
    !variants?.length ||
    !variants.every(
      (variant) => variant.properties?.[discriminatorKey]?.const !== undefined,
    )
  )
    return undefined;
  return variants;
};

export const variantTag = (variant: SchemaNode) =>
  String(variant.properties?.[discriminatorKey]?.const);

/** Constant choices of a unit-variant enum such as `kind`. */
export const constantChoices = (node: SchemaNode) =>
  node.oneOf?.every((option) => option.const !== undefined)
    ? node.oneOf
    : undefined;

/** Literal choices for a value: constants, enums, and booleans. */
export const choicesOf = (node: SchemaNode): string[] | undefined => {
  if (node.const !== undefined) return [String(node.const)];
  if (node.enum) return node.enum.map(String);
  const constants = constantChoices(node);
  if (constants) return constants.map((option) => String(option.const));
  if (primaryType(node) === 'boolean') return ['true', 'false'];
  return undefined;
};

/** A short type name such as `string`, `table`, or `string[]`. */
export const typeLabel = (root: SchemaNode, node: SchemaNode): string => {
  if (node.const !== undefined) return JSON.stringify(node.const);
  const type = primaryType(node);
  if (type === 'array' && node.items)
    return `${typeLabel(root, unwrapSchema(root, node.items))}[]`;
  if (type === 'object' || node.oneOf) return 'table';
  return type ?? 'value';
};

export type ResolvedSchema = {
  node: SchemaNode;
  /** Candidate variants when a tagged union has no (valid) `type` yet. */
  variants?: SchemaNode[];
};

/**
 * Resolves the schema at a TOML path, selecting tagged-union variants from
 * the `type` values already written in the document.
 */
export const schemaAtPath = (
  root: SchemaNode,
  path: TomlPath,
  valueAt: (path: TomlPath) => unknown,
): ResolvedSchema | undefined => {
  let resolved = selectVariant(root, unwrapSchema(root, root), [], valueAt);
  for (let index = 0; index < path.length; index++) {
    const segment = path[index];
    const { node, variants } = resolved;
    let next: SchemaNode | undefined;
    if (typeof segment === 'number') {
      next = node.items;
    } else if (variants) {
      if (segment === discriminatorKey) {
        return {
          node: {
            oneOf: variants.map((variant) => ({
              const: variantTag(variant),
              description: variant.description,
            })),
            type: 'string',
          },
        };
      }
      next = variants.find((variant) => variant.properties?.[segment])
        ?.properties?.[segment];
    } else {
      next =
        node.properties?.[segment] ??
        (typeof node.additionalProperties === 'object'
          ? node.additionalProperties
          : undefined);
    }
    if (!next) return undefined;
    resolved = selectVariant(
      root,
      unwrapSchema(root, next),
      path.slice(0, index + 1),
      valueAt,
    );
  }
  return resolved;
};

const selectVariant = (
  root: SchemaNode,
  node: SchemaNode,
  path: TomlPath,
  valueAt: (path: TomlPath) => unknown,
): ResolvedSchema => {
  const variants = taggedVariants(root, node);
  if (!variants) return { node };
  const tag = valueAt([...path, discriminatorKey]);
  const selected = variants.find((variant) => variantTag(variant) === tag);
  return selected ? { node: selected } : { node, variants };
};
