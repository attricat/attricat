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
  scanYaml,
  type TextRange,
  type YamlCursor,
  type YamlOutline,
} from '../../components/yamlOutline';
import {
  yamlFlowMapping,
  yamlKey,
  yamlString,
  yamlValue,
} from '../../components/yamlSyntax';
import {
  componentCandidates,
  editorCandidates,
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
  outline: YamlOutline;
  references: DefinitionReferences;
  root: SchemaNode;
};

/** Values of these types are written without quotes. */
const unquotedTypes = new Set(['boolean', 'integer', 'number']);

/** Spaces per nesting level in inserted YAML. */
const indentUnit = '  ';

const escapeSnippet = (value: string) => value.replace(/[$}\\]/g, '\\$&');
const escapeChoice = (value: string) => value.replace(/[$}\\|,]/g, '\\$&');
const choiceList = (values: string[]) =>
  values.map((value) => escapeChoice(yamlString(value))).join(',');

/** Numbers snippet tab stops across the placeholders of one insertion. */
class TabStops {
  private next = 1;
  take() {
    return this.next++;
  }
}

const isMapping = (node: SchemaNode) =>
  primaryType(node) === 'object' || Boolean(node.oneOf);

const indented = (lines: string[]) => lines.map((line) => indentUnit + line);

/** Lines of a sequence item: `- ` before the first, aligned after it. */
const itemLines = (body: string[]) =>
  body.length
    ? body.map((line, index) => (index ? indentUnit : '- ') + line)
    : ['- '];

/** A placeholder for a value written after `key: ` on the same line. */
const placeholder = (
  root: SchemaNode,
  node: SchemaNode,
  stops: TabStops,
): string => {
  if (node.const !== undefined) return yamlValue(node.const);
  const type = primaryType(node);
  const choices = choicesOf(node);
  if (choices && type === 'boolean')
    return `\${${stops.take()}|${choices.join(',')}|}`;
  if (choices) return `\${${stops.take()}|${choiceList(choices)}|}`;
  switch (type) {
    case 'array':
      return `[\${${stops.take()}}]`;
    case 'object': {
      const fields = requiredLines(root, node, new Set(), stops);
      return yamlFlowMapping(fields.length ? fields : [`\${${stops.take()}}`]);
    }
    default:
      return `\${${stops.take()}}`;
  }
};

/** `key: placeholder` lines for a mapping's required scalar keys. */
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
    if (isMapping(unwrapped) || (items && isMapping(items))) return [];
    return [`${yamlKey(key)}: ${placeholder(root, unwrapped, stops)}`];
  });

const variantBody = (
  root: SchemaNode,
  variant: SchemaNode,
  stops: TabStops,
) => [
  `${discriminatorKey}: ${yamlString(variantTag(variant))}`,
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

type KeyCursor = Extract<YamlCursor, { kind: 'key' }>;

/**
 * Joins inserted lines. Monaco indents every line after the first like the
 * cursor's line; after a `- `, the rest also align with the item's first key.
 */
const joinLines = (cursor: KeyCursor, lines: string[]) =>
  cursor.inline
    ? lines.join(', ')
    : lines.join(cursor.item ? `\n${indentUnit}` : '\n');

const propertySuggestions = (
  root: SchemaNode,
  cursor: KeyCursor,
  key: string,
  property: SchemaNode,
  required: boolean,
): DefinitionSuggestion[] => {
  const name = yamlKey(key);
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
  // Maps of mappings read best as one block of named entries.
  const namedMappings = mapValue && isMapping(mapValue) ? mapValue : undefined;

  if (!cursor.inline) {
    if (items && isMapping(items)) {
      if (itemVariants)
        return variantSuggestions(
          root,
          itemVariants,
          cursor.range,
          (body) =>
            joinLines(cursor, [`${name}:`, ...indented(itemLines(body))]),
          (tag) => `${key}: ${tag}`,
        ).map((suggestion) => ({
          ...base,
          ...suggestion,
          sortText: base.sortText,
        }));
      const body = requiredLines(root, items, new Set(), new TabStops());
      return [
        {
          ...base,
          insertText: joinLines(cursor, [
            `${name}:`,
            ...indented(itemLines(body.length ? body : ['$1'])),
          ]),
          kind: suggestionKinds.section,
          label: key,
          snippet: true,
        },
      ];
    }
    if (namedMappings) {
      const stops = new TabStops();
      const keyChoices = property[schemaExtensions.keySuggestions];
      const entry = keyChoices?.length
        ? `\${${stops.take()}|${choiceList(keyChoices)}|}`
        : `\${${stops.take()}:${i18n.t('definitionEditor.placeholders.name')}}`;
      const variants = taggedVariants(root, namedMappings);
      const body = variants
        ? [
            `${discriminatorKey}: \${${stops.take()}|${choiceList(variants.map(variantTag))}|}`,
          ]
        : [`\${${stops.take()}}`];
      return [
        {
          ...base,
          insertText: joinLines(cursor, [
            `${name}:`,
            `${indentUnit}${entry}:`,
            ...indented(indented(body)),
          ]),
          kind: suggestionKinds.section,
          label: key,
          snippet: true,
        },
      ];
    }
    if (primaryType(property) === 'object') {
      const body = requiredLines(root, property, new Set(), new TabStops());
      return [
        {
          ...base,
          insertText: joinLines(cursor, [
            `${name}:`,
            ...indented(body.length ? body : ['$1']),
          ]),
          kind: suggestionKinds.section,
          label: key,
          snippet: true,
        },
      ];
    }
  }

  const stops = new TabStops();
  const value = mapValue
    ? yamlFlowMapping([`\${${stops.take()}}`])
    : items && isMapping(items)
      ? `[${yamlFlowMapping([`\${${stops.take()}}`])}]`
      : placeholder(root, property, stops);
  const hinted = Boolean(
    property[schemaExtensions.reference] ??
    property[schemaExtensions.suggestions] ??
    items?.[schemaExtensions.reference],
  );
  return [
    {
      ...base,
      insertText: `${name}: ${value}`,
      kind: suggestionKinds.property,
      label: key,
      snippet: true,
      triggerSuggest: hinted && !choicesOf(property),
    },
  ];
};

const keySuggestions = async (
  context: Context,
  cursor: KeyCursor,
  prefix: string,
): Promise<DefinitionSuggestion[]> => {
  const { outline, root } = context;
  const resolved = schemaAtPath(root, cursor.mapping, outline.valueAt);
  if (!resolved) return [];
  // A `- ` item of a sequence of scalars takes a value, not a key.
  if (
    cursor.item &&
    !resolved.variants &&
    !isMapping(resolved.node) &&
    typeof cursor.mapping.at(-1) === 'number'
  )
    return valueSuggestions(
      context,
      {
        flow: false,
        kind: 'value',
        path: cursor.mapping,
        quoted: false,
        range: cursor.range,
      },
      prefix,
    );
  const present = outline.keysIn(cursor.mapping);
  const variantLabel = (tag: string) => `${discriminatorKey}: ${tag}`;
  if (resolved.variants && !resolved.defaulted) {
    return variantSuggestions(
      root,
      resolved.variants,
      cursor.range,
      (body) => joinLines(cursor, body),
      variantLabel,
    );
  }
  const { node } = resolved;
  // An empty defaulted mapping can still become one of the tagged variants.
  const alternatives =
    resolved.defaulted && resolved.variants && !present.size
      ? variantSuggestions(
          root,
          resolved.variants,
          cursor.range,
          (body) => joinLines(cursor, body),
          variantLabel,
        )
      : [];
  const valueType = outline.valueAt([...cursor.mapping, valueTypeKey]);
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

/** Narrows a replacement range to the dotted segment being typed. */
const segmentRange = (range: TextRange, prefix: string): TextRange => {
  const separator = prefix.lastIndexOf(attributePathSeparator);
  return separator < 0
    ? range
    : { ...range, start: range.start + separator + 1 };
};

async function valueSuggestions(
  context: Context,
  cursor: Extract<YamlCursor, { kind: 'value' }>,
  prefix: string,
): Promise<DefinitionSuggestion[]> {
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
      (body) => (wrap ? `[${yamlFlowMapping(body)}]` : yamlFlowMapping(body)),
      (tag) => yamlFlowMapping([`${discriminatorKey}: ${tag}`]),
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
      insertText: yamlValue({ id: component.id, version: component.version }),
      kind: suggestionKinds.reference,
      label: component.id,
      range: cursor.range,
      snippet: false,
    }));
  }

  if (
    !cursor.quoted &&
    node.properties?.id?.[schemaExtensions.reference] ===
      definitionReferenceKinds.attributeEditor
  ) {
    return (await editorCandidates(outline, context.references, path)).map(
      (editor) => ({
        detail: i18n.t('definitionEditor.details.editor', {
          extension: editor.extensionName,
          version: editor.version,
        }),
        filterText: editor.id,
        insertText: yamlValue({ id: editor.id, version: editor.version }),
        kind: suggestionKinds.reference,
        label: editor.id,
        range: cursor.range,
        snippet: false,
      }),
    );
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
      const flow = cursor.flow || wrap;
      const literal = quote
        ? candidate.continues
          ? yamlString(raw, flow) === raw
            ? `${escapeSnippet(raw)}$0`
            : `'${escapeSnippet(raw.replaceAll("'", "''"))}$0'`
          : yamlString(raw, flow)
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
}

export const definitionCompletions = async ({
  kind,
  offset,
  references,
  text,
}: CompletionInput): Promise<DefinitionSuggestion[]> => {
  const outline = scanYaml(text, offset);
  const context: Context = {
    outline,
    references,
    root: definitionSchemas[kind],
  };
  const { cursor } = outline;
  switch (cursor?.kind) {
    case 'key':
      return keySuggestions(
        context,
        cursor,
        text.slice(cursor.range.start, offset),
      );
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
