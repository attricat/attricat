import i18n from '../../i18n';
import {
  attributePathSeparator,
  discriminatorKey,
  optionalSortPrefix,
  requiredSortPrefix,
  schemaExtensions,
  valueTypeKey,
  type DefinitionKind,
} from './constants';
import type { DefinitionReferences } from './definitionReferences';
import {
  choicesOf,
  constantChoices,
  definitionReferenceKinds,
  definitionSchemas,
  primaryType,
  schemaAtPath,
  taggedVariants,
  typeLabel,
  unwrapSchema,
  variantTag,
  type SchemaNode,
} from './definitionSchema';
import {
  scanToml,
  type TextRange,
  type TomlCursor,
  type TomlOutline,
} from '../../components/tomlOutline';
import {
  componentCandidates,
  referenceCandidates,
  type Candidate,
} from './definitionReferenceCandidates';

export const suggestionKinds = {
  property: 'property',
  reference: 'reference',
  section: 'section',
  value: 'value',
  variant: 'variant',
} as const;
export type SuggestionKind =
  (typeof suggestionKinds)[keyof typeof suggestionKinds];

export type DefinitionSuggestion = {
  detail?: string;
  documentation?: string;
  filterText?: string;
  insertText: string;
  kind: SuggestionKind;
  label: string;
  range: TextRange;
  /** Whether `insertText` uses Monaco snippet syntax. */
  snippet: boolean;
  sortText?: string;
  /** Reopens suggestions after insertion, for example after `category.`. */
  triggerSuggest?: boolean;
};

type CompletionInput = {
  kind: DefinitionKind;
  offset: number;
  references: DefinitionReferences;
  text: string;
};

type Context = {
  outline: TomlOutline;
  references: DefinitionReferences;
  root: SchemaNode;
};

/** Values of these types are written without quotes. */
const unquotedTypes = new Set(['boolean', 'integer', 'number']);

const isString = (segment: string | number): segment is string =>
  typeof segment === 'string';

const bareKey = /^[A-Za-z0-9_-]+$/;
const tomlKey = (key: string) =>
  bareKey.test(key) ? key : JSON.stringify(key);
const tomlString = (value: string) => JSON.stringify(value);
const escapeSnippet = (value: string) => value.replace(/[$}\\]/g, '\\$&');
const escapeChoice = (value: string) => value.replace(/[$}\\|,]/g, '\\$&');

/** Numbers snippet tab stops across the placeholders of one insertion. */
class TabStops {
  private next = 1;
  take() {
    return this.next++;
  }
}

const isTable = (node: SchemaNode) =>
  primaryType(node) === 'object' || Boolean(node.oneOf);

/** Whether a table holds nested tables, so it reads best as a `[section]`. */
const hasNestedTables = (root: SchemaNode, node: SchemaNode) =>
  Object.values(node.properties ?? {}).some((property) => {
    const unwrapped = unwrapSchema(root, property);
    const items = unwrapped.items && unwrapSchema(root, unwrapped.items);
    return isTable(unwrapped) || (items !== undefined && isTable(items));
  });

const placeholder = (
  root: SchemaNode,
  node: SchemaNode,
  stops: TabStops,
): string => {
  if (node.const !== undefined) return JSON.stringify(node.const);
  const type = primaryType(node);
  const choices = choicesOf(node);
  if (choices && type === 'boolean')
    return `\${${stops.take()}|${choices.join(',')}|}`;
  if (choices)
    return `"\${${stops.take()}|${choices.map(escapeChoice).join(',')}|}"`;
  switch (type) {
    case 'string':
      return `"\${${stops.take()}}"`;
    case 'array': {
      const items = node.items && unwrapSchema(root, node.items);
      return items && primaryType(items) === 'string'
        ? `["\${${stops.take()}}"]`
        : `[\${${stops.take()}}]`;
    }
    case 'object': {
      const fields = requiredLines(root, node, new Set(), stops);
      return fields.length
        ? `{ ${fields.join(', ')} }`
        : `{ \${${stops.take()}} }`;
    }
    default:
      return `\${${stops.take()}}`;
  }
};

/** `key = placeholder` lines for a table's required scalar keys. */
const requiredLines = (
  root: SchemaNode,
  node: SchemaNode,
  skip: Set<string>,
  stops: TabStops,
) =>
  (node.required ?? []).flatMap((key) => {
    const property = node.properties?.[key];
    if (skip.has(key) || !property) return [];
    const unwrapped = unwrapSchema(root, property);
    const items = unwrapped.items && unwrapSchema(root, unwrapped.items);
    if (isTable(unwrapped) || (items && isTable(items))) return [];
    return [`${tomlKey(key)} = ${placeholder(root, unwrapped, stops)}`];
  });

const variantBody = (
  root: SchemaNode,
  variant: SchemaNode,
  stops: TabStops,
) => [
  `${discriminatorKey} = ${tomlString(variantTag(variant))}`,
  ...requiredLines(root, variant, new Set([discriminatorKey]), stops),
];

const variantSuggestions = (
  root: SchemaNode,
  variants: SchemaNode[],
  range: TextRange,
  format: (body: string[], tag: string) => string,
  labelFor: (tag: string) => string,
): DefinitionSuggestion[] =>
  variants.map((variant) => {
    const tag = variantTag(variant);
    return {
      detail: i18n.t('definitionEditor.details.block'),
      documentation: variant.description,
      filterText: labelFor(tag),
      insertText: format(variantBody(root, variant, new TabStops()), tag),
      kind: suggestionKinds.variant,
      label: labelFor(tag),
      range,
      snippet: true,
    };
  });

const propertySuggestions = (
  root: SchemaNode,
  cursor: Extract<TomlCursor, { kind: 'key' }>,
  key: string,
  property: SchemaNode,
  required: boolean,
): DefinitionSuggestion[] => {
  const names = [...cursor.table.filter(isString), key].map(tomlKey);
  const base = {
    detail: required
      ? i18n.t('definitionEditor.details.requiredType', {
          type: typeLabel(root, property),
        })
      : typeLabel(root, property),
    documentation: property.description,
    filterText: key,
    range: cursor.range,
    sortText: `${required ? requiredSortPrefix : optionalSortPrefix}${key}`,
  };
  const items = property.items && unwrapSchema(root, property.items);
  const itemVariants = items && taggedVariants(root, items);
  const mapValue =
    typeof property.additionalProperties === 'object'
      ? unwrapSchema(root, property.additionalProperties)
      : undefined;
  // Maps of scalars read best as one `[table]` of keys, not a table per key.
  const tableMap = mapValue && isTable(mapValue) ? mapValue : undefined;

  if (!cursor.inline) {
    if (items && isTable(items)) {
      const header = `[[${names.join('.')}]]`;
      if (itemVariants)
        return variantSuggestions(
          root,
          itemVariants,
          cursor.range,
          (body) => [header, ...body].join('\n'),
          (tag) => `${header} ${tag}`,
        ).map((suggestion) => ({
          ...base,
          ...suggestion,
          sortText: base.sortText,
        }));
      const stops = new TabStops();
      return [
        {
          ...base,
          insertText: [
            header,
            ...requiredLines(root, items, new Set(), stops),
          ].join('\n'),
          kind: suggestionKinds.section,
          label: header,
          snippet: true,
        },
      ];
    }
    if (tableMap) {
      const stops = new TabStops();
      const keyChoices = property[schemaExtensions.keySuggestions];
      const name = keyChoices?.length
        ? `\${${stops.take()}|${keyChoices.map(escapeChoice).join(',')}|}`
        : `\${${stops.take()}:${i18n.t('definitionEditor.placeholders.name')}}`;
      const variants = taggedVariants(root, tableMap);
      const body = variants
        ? [
            `${discriminatorKey} = "\${${stops.take()}|${variants.map((variant) => escapeChoice(variantTag(variant))).join(',')}|}"`,
          ]
        : [];
      return [
        {
          ...base,
          insertText: [`[${names.join('.')}.${name}]`, ...body].join('\n'),
          kind: suggestionKinds.section,
          label: `[${names.join('.')}.…]`,
          snippet: true,
        },
      ];
    }
    if (primaryType(property) === 'object' && hasNestedTables(root, property)) {
      const header = `[${names.join('.')}]`;
      return [
        {
          ...base,
          insertText: [
            header,
            ...requiredLines(root, property, new Set(), new TabStops()),
          ].join('\n'),
          kind: suggestionKinds.section,
          label: header,
          snippet: true,
        },
      ];
    }
  }

  const stops = new TabStops();
  const value = mapValue
    ? `{ \${${stops.take()}} }`
    : items && isTable(items)
      ? `[{ \${${stops.take()}} }]`
      : placeholder(root, property, stops);
  const hinted = Boolean(
    property[schemaExtensions.reference] ??
    property[schemaExtensions.suggestions] ??
    items?.[schemaExtensions.reference],
  );
  return [
    {
      ...base,
      insertText: `${tomlKey(key)} = ${value}`,
      kind: suggestionKinds.property,
      label: key,
      snippet: true,
      triggerSuggest: hinted && !choicesOf(property),
    },
  ];
};

const keySuggestions = (
  { outline, root }: Context,
  cursor: Extract<TomlCursor, { kind: 'key' }>,
): DefinitionSuggestion[] => {
  const resolved = schemaAtPath(root, cursor.table, outline.valueAt);
  if (!resolved) return [];
  const present = outline.keysIn(cursor.table);
  if (resolved.variants && !resolved.defaulted) {
    const separator = cursor.inline ? ', ' : '\n';
    return variantSuggestions(
      root,
      resolved.variants,
      cursor.range,
      (body) => body.join(separator),
      (tag) => `${discriminatorKey} = ${tomlString(tag)}`,
    );
  }
  const { node } = resolved;
  // An empty defaulted table can still become one of the tagged variants.
  const alternatives =
    resolved.defaulted && resolved.variants && !present.size
      ? variantSuggestions(
          root,
          resolved.variants,
          cursor.range,
          (body) => body.join(cursor.inline ? ', ' : '\n'),
          (tag) => `${discriminatorKey} = ${tomlString(tag)}`,
        )
      : [];
  const valueType = outline.valueAt([...cursor.table, valueTypeKey]);
  const required = new Set(node.required);
  const properties = Object.entries(node.properties ?? {});
  if (typeof node.additionalProperties === 'object') {
    for (const key of node[schemaExtensions.keySuggestions] ?? [])
      properties.push([key, node.additionalProperties]);
  }
  return [
    ...properties.flatMap(([key, schema]) => {
      if (present.has(key)) return [];
      const property = unwrapSchema(root, schema);
      const valueTypes = property[schemaExtensions.valueTypes];
      if (
        valueTypes &&
        typeof valueType === 'string' &&
        !valueTypes.includes(valueType)
      )
        return [];
      return propertySuggestions(
        root,
        cursor,
        key,
        property,
        required.has(key),
      );
    }),
    ...alternatives,
  ];
};

const headerSuggestions = (
  { outline, root }: Context,
  cursor: Extract<TomlCursor, { kind: 'header' }>,
  text: string,
): DefinitionSuggestion[] => {
  const closing = cursor.array ? ']]' : ']';
  const closed = text.startsWith(closing, cursor.range.end);
  const tables = cursor.table.length ? [[], cursor.table] : [[]];
  const suggestions = new Map<string, DefinitionSuggestion>();
  for (const table of tables) {
    const resolved = schemaAtPath(root, table, outline.valueAt);
    if (!resolved || resolved.variants) continue;
    const names = table.filter(isString).map(tomlKey);
    for (const [key, schema] of Object.entries(
      resolved.node.properties ?? {},
    )) {
      const property = unwrapSchema(root, schema);
      const items = property.items && unwrapSchema(root, property.items);
      const isArrayTable = items !== undefined && isTable(items);
      const isMap = typeof property.additionalProperties === 'object';
      const isSection =
        isMap || (primaryType(property) === 'object' && !isArrayTable);
      if (cursor.array ? !isArrayTable : !isSection) continue;
      const name = [...names, tomlKey(key)].join('.');
      const keyChoices = property[schemaExtensions.keySuggestions];
      const insertName = isMap
        ? `${escapeSnippet(name)}.${keyChoices?.length ? `\${1|${keyChoices.map(escapeChoice).join(',')}|}` : `\${1:${i18n.t('definitionEditor.placeholders.name')}}`}`
        : escapeSnippet(name);
      suggestions.set(name, {
        detail: i18n.t('definitionEditor.details.section'),
        documentation: property.description,
        insertText: `${insertName}${closed ? '' : closing}`,
        kind: suggestionKinds.section,
        label: isMap ? `${name}.…` : name,
        range: cursor.range,
        snippet: true,
      });
    }
  }
  return [...suggestions.values()];
};

/** Narrows a replacement range to the dotted segment being typed. */
const segmentRange = (range: TextRange, prefix: string): TextRange => {
  const separator = prefix.lastIndexOf(attributePathSeparator);
  return separator < 0
    ? range
    : { ...range, start: range.start + separator + 1 };
};

const valueSuggestions = async (
  context: Context,
  cursor: Extract<TomlCursor, { kind: 'value' }>,
  prefix: string,
): Promise<DefinitionSuggestion[]> => {
  const { outline, root } = context;
  const resolved = schemaAtPath(root, cursor.path, outline.valueAt);
  if (!resolved) return [];
  let { node } = resolved;
  let { path } = cursor;
  let variants = resolved.variants;
  const wrap = primaryType(node) === 'array' && !cursor.quoted;
  // Array fields carry their hints on the array, not on each element.
  const array = wrap
    ? node
    : typeof path.at(-1) === 'number'
      ? schemaAtPath(root, path.slice(0, -1), outline.valueAt)?.node
      : undefined;
  if (wrap && node.items) {
    node = unwrapSchema(root, node.items);
    path = [...path, 0];
    variants = taggedVariants(root, node);
  }

  if (!cursor.quoted && variants) {
    return variantSuggestions(
      root,
      variants,
      cursor.range,
      (body) => (wrap ? `[{ ${body.join(', ')} }]` : `{ ${body.join(', ')} }`),
      (tag) => `{ ${discriminatorKey} = ${tomlString(tag)} }`,
    );
  }

  if (
    !cursor.quoted &&
    node.properties?.id?.[schemaExtensions.reference] ===
      definitionReferenceKinds.viewComponent
  ) {
    return componentCandidates(outline, path).map((component) => ({
      detail: i18n.t('definitionEditor.details.component', {
        version: component.version,
      }),
      filterText: component.id,
      insertText: `{ id = ${tomlString(component.id)}, version = ${component.version} }`,
      kind: suggestionKinds.reference,
      label: component.id,
      range: cursor.range,
      snippet: false,
    }));
  }

  const reference =
    node[schemaExtensions.reference] ?? array?.[schemaExtensions.reference];
  const choices = constantChoices(node);
  const candidates: Candidate[] = choices
    ? choices.map((choice) => ({
        documentation: choice.description,
        value: String(choice.const),
      }))
    : [
        ...(choicesOf(node) ?? []).map((value) => ({ value })),
        ...(
          node[schemaExtensions.suggestions] ??
          array?.[schemaExtensions.suggestions] ??
          []
        ).map((value) => ({ value })),
        ...(reference
          ? await referenceCandidates(context, reference, path, prefix)
          : []),
      ];

  const literalType = unquotedTypes.has(primaryType(node) ?? '');
  const range =
    cursor.quoted &&
    (reference === definitionReferenceKinds.attributePath ||
      reference === definitionReferenceKinds.relationshipPath ||
      reference === definitionReferenceKinds.includeAttribute)
      ? segmentRange(cursor.range, prefix)
      : cursor.range;
  const segmentOffset = range.start - cursor.range.start;
  return candidates.map((candidate) => {
    const raw = String(candidate.value);
    const quote = !literalType && typeof candidate.value === 'string';
    const text = cursor.quoted ? raw.slice(segmentOffset) : raw;
    let insertText = text;
    let snippet = false;
    if (!cursor.quoted) {
      const literal = quote
        ? candidate.continues
          ? `"${escapeSnippet(raw)}$0"`
          : tomlString(raw)
        : raw;
      insertText = wrap ? `[${literal}]` : literal;
      snippet = Boolean(candidate.continues);
    }
    return {
      detail: candidate.detail,
      documentation: candidate.documentation,
      filterText: cursor.quoted ? text : raw,
      insertText,
      kind: reference ? suggestionKinds.reference : suggestionKinds.value,
      label: candidate.label ?? raw,
      range,
      snippet,
      sortText: candidate.sortText,
      triggerSuggest: candidate.continues,
    };
  });
};

export const definitionCompletions = async ({
  kind,
  offset,
  references,
  text,
}: CompletionInput): Promise<DefinitionSuggestion[]> => {
  const outline = scanToml(text, offset);
  const context: Context = {
    outline,
    references,
    root: definitionSchemas[kind],
  };
  const { cursor } = outline;
  switch (cursor?.kind) {
    case 'key':
      return keySuggestions(context, cursor);
    case 'header':
      return headerSuggestions(context, cursor, text);
    case 'value':
      return valueSuggestions(
        context,
        cursor,
        text.slice(cursor.range.start, offset),
      );
    default:
      return [];
  }
};
