import {
  scanYaml,
  type TextRange,
  type YamlEntry,
  type YamlPath,
} from '../../components/yamlOutline';
import {
  aliasKey,
  codeKey,
  discriminatorKey,
  fieldKey,
  labelKey,
  nameKey,
} from './constants';

export const definitionSymbolKinds = {
  /** A mapping under a key, such as `views`. */
  object: 'object',
  /** A sequence under a key, such as `attributes`. */
  array: 'array',
  /** A mapping inside a sequence, such as one attribute. */
  item: 'item',
  /** A top-level scalar, such as `code`. */
  property: 'property',
} as const;
export type DefinitionSymbolKind =
  (typeof definitionSymbolKinds)[keyof typeof definitionSymbolKinds];

/** A part of a definition for the editor's outline and sticky scroll. */
export type DefinitionSymbol = {
  children: DefinitionSymbol[];
  /** What kind of block an item is, or a property's value. */
  detail?: string;
  kind: DefinitionSymbolKind;
  name: string;
  /** All of it, including nested content. */
  range: TextRange;
  /** The key, or an item's first line. */
  selectionRange: TextRange;
};

/** Keys that name a sequence item, in the order they are preferred. */
const itemNameKeys = [codeKey, aliasKey, fieldKey, labelKey, nameKey];

const pathKey = (path: YamlPath) => JSON.stringify(path);

/**
 * The definition's mappings and sequences as a tree. Items are named by
 * their code, alias, field, label or name, and blocks by their type.
 */
export const definitionSymbols = (text: string): DefinitionSymbol[] => {
  const outline = scanYaml(text);
  const collections = new Map(
    outline.collections.map((collection) => [
      pathKey(collection.path),
      collection,
    ]),
  );
  const childEntries = new Map<string, YamlEntry[]>();
  for (const entry of outline.entries) {
    const parent = pathKey(entry.path.slice(0, -1));
    childEntries.set(parent, [...(childEntries.get(parent) ?? []), entry]);
  }
  const symbolsUnder = (path: YamlPath): DefinitionSymbol[] =>
    (childEntries.get(pathKey(path)) ?? []).flatMap((entry) => {
      const key = entry.path[entry.path.length - 1];
      const collection = collections.get(pathKey(entry.path));
      if (typeof key === 'number') {
        if (collection?.kind !== 'map') return [];
        const type = outline.valueAt([...entry.path, discriminatorKey]);
        const name = itemNameKeys
          .map((nameKey) => outline.valueAt([...entry.path, nameKey]))
          .find((value) => value !== undefined);
        return {
          children: symbolsUnder(entry.path),
          detail: name === undefined ? undefined : type?.toString(),
          kind: definitionSymbolKinds.item,
          name: String(name ?? type ?? key + 1),
          range: collection.range,
          selectionRange: entry.range,
        };
      }
      if (collection)
        return {
          children: symbolsUnder(entry.path),
          kind:
            collection.kind === 'map'
              ? definitionSymbolKinds.object
              : definitionSymbolKinds.array,
          name: key,
          range: { end: collection.range.end, start: entry.range.start },
          selectionRange: entry.range,
        };
      // Nested scalars would crowd the outline; top-level ones name it.
      if (path.length > 0 || entry.value === undefined) return [];
      return {
        children: [],
        detail: String(entry.value),
        kind: definitionSymbolKinds.property,
        name: key,
        range: {
          end: entry.valueRange?.end ?? entry.range.end,
          start: entry.range.start,
        },
        selectionRange: entry.range,
      };
    });
  return symbolsUnder([]);
};
