import type { Node as JsonNode } from 'jsonc-parser';
import i18n from '../../i18n';
import { discriminatorKey, type DefinitionKind } from './constants';
import { embeddedJson } from './definitionEmbeddedReferences';
import {
  choicesOf,
  definitionSchemas,
  schemaAtPath,
  typeLabel,
  type SchemaNode,
} from './definitionSchema';
import { embeddedJsonSchemas } from './embeddedJsonSchema';
import {
  scanYaml,
  type TextRange,
  type YamlEntry,
  type YamlOutline,
  type YamlPath,
} from '../../components/yamlOutline';

type DefinitionHover = { contents: string[]; range: TextRange };

/** The keys and values to describe, from YAML or from embedded JSON. */
type HoverDocument = Pick<YamlOutline, 'entries' | 'valueAt'>;

const contains = (range: TextRange, offset: number) =>
  offset >= range.start && offset <= range.end;

const pathKey = (path: YamlPath) => JSON.stringify(path);

/** Entries like the YAML outline's for JSON parsed at `start` in the text. */
const jsonDocument = (root: JsonNode, start: number): HoverDocument => {
  const entries: YamlEntry[] = [];
  const toRange = ({ length, offset }: JsonNode) => ({
    end: start + offset + length,
    start: start + offset,
  });
  const scalar = (node: JsonNode) =>
    node.type === 'string' || node.type === 'number' || node.type === 'boolean'
      ? (node.value as string | number | boolean)
      : undefined;
  const walk = (node: JsonNode, path: YamlPath) => {
    if (node.type === 'object')
      for (const property of node.children ?? []) {
        const [key, value] = property.children ?? [];
        if (typeof key?.value !== 'string') continue;
        const childPath = [...path, key.value];
        entries.push({
          path: childPath,
          range: toRange(key),
          value: value && scalar(value),
          valueRange:
            value && scalar(value) !== undefined ? toRange(value) : undefined,
        });
        if (value) walk(value, childPath);
      }
    else if (node.type === 'array')
      (node.children ?? []).forEach((item, index) => {
        const childPath = [...path, index];
        const value = scalar(item);
        entries.push({
          path: childPath,
          range: toRange(item),
          value,
          valueRange: value === undefined ? undefined : toRange(item),
        });
        walk(item, childPath);
      });
  };
  walk(root, []);
  const byPath = new Map(entries.map((entry) => [pathKey(entry.path), entry]));
  return { entries, valueAt: (path) => byPath.get(pathKey(path))?.value };
};

/** What a chosen value means, when its options are documented. */
const valueDescription = (
  root: SchemaNode,
  document: HoverDocument,
  path: YamlPath,
  node: SchemaNode,
  value: unknown,
) => {
  const option = node.oneOf?.find(
    (choice) => choice.const !== undefined && String(choice.const) === value,
  );
  if (option?.description) return option.description;
  // A tagged union documents each variant on the variant, not its tag.
  if (path.at(-1) !== discriminatorKey) return undefined;
  return schemaAtPath(root, path.slice(0, -1), document.valueAt)?.node
    .description;
};

const describe = (
  root: SchemaNode,
  document: HoverDocument,
  { path, range, value, valueRange }: YamlEntry,
  onValue: boolean,
): DefinitionHover | undefined => {
  // Items of a sequence are described by the sequence.
  const described = typeof path.at(-1) === 'number' ? path.slice(0, -1) : path;
  const node = schemaAtPath(root, described, document.valueAt)?.node;
  if (!node) return undefined;
  const name = described
    .filter((segment) => typeof segment === 'string')
    .at(-1);
  const choices = choicesOf(node);
  const chosen =
    onValue && value !== undefined
      ? valueDescription(root, document, described, node, String(value))
      : undefined;
  const contents = [
    // Free text needs no repeating; a chosen option says what it means.
    onValue && value !== undefined && (chosen || choices)
      ? `**${String(value)}**${chosen ? ` — ${chosen}` : ''}`
      : undefined,
    `**${name ?? ''}** \`${typeLabel(root, node)}\``,
    node.description,
    choices &&
      i18n.t('definitionEditor.hover.allowedValues', {
        values: choices.map((choice) => `\`${choice}\``).join(', '),
      }),
    node.default !== undefined &&
      node.default !== null &&
      i18n.t('definitionEditor.hover.default', {
        value: `\`${JSON.stringify(node.default)}\``,
      }),
  ].filter((content): content is string => Boolean(content));
  return { contents, range: onValue && valueRange ? valueRange : range };
};

/** Describes the key or value at `offset` from its contract. */
const describeAt = (
  root: SchemaNode,
  document: HoverDocument,
  offset: number,
) => {
  const key = document.entries.find(
    ({ path, range }) =>
      typeof path.at(-1) === 'string' && contains(range, offset),
  );
  if (key) return describe(root, document, key, false);
  const value = document.entries.find(
    ({ valueRange }) => valueRange && contains(valueRange, offset),
  );
  return value && describe(root, document, value, true);
};

/**
 * Documents the key or value under the cursor from its contract, including
 * inside the JSON Schemas that `record_schema` and `value_schema` hold.
 */
export const definitionHover = (
  kind: DefinitionKind,
  text: string,
  offset: number,
): DefinitionHover | undefined => {
  const outline = scanYaml(text);
  const holder = outline.entries.find(
    ({ valueRange }) => valueRange && contains(valueRange, offset),
  );
  const json = holder && embeddedJson(text, holder);
  if (json)
    return describeAt(
      embeddedJsonSchemas()[json.kind],
      jsonDocument(json.root, json.start),
      offset,
    );
  return describeAt(definitionSchemas[kind], outline, offset);
};
