import { definitionKinds, type DefinitionKind } from './constants';
import {
  scanYaml,
  type YamlCollection,
  type YamlOutline,
  type YamlPath,
} from '../../components/yamlOutline';
import { yamlKey } from '../../components/yamlSyntax';

const snippetPlacements = {
  /** Root `key: value` lines, inserted after the last such line. */
  key: 'key',
  /** A `key:` block in the `parent` mapping, appended to it. */
  entry: 'entry',
  /** A mapping appended as a new item of the `parent` sequence. */
  item: 'item',
} as const;
type SnippetPlacement =
  (typeof snippetPlacements)[keyof typeof snippetPlacements];

export type DefinitionSnippet = {
  /** Translation key suffix under `definitionEditor.snippets`. */
  id: string;
  /** Translation key suffix under `definitionEditor.snippetGroups`. */
  group: string;
  /**
   * YAML written at the indentation of its parent's children. Monaco snippet
   * syntax: `${1:default}` tab stops, `${1|a,b|}` choices.
   */
  body: string;
  placement: SnippetPlacement;
  /** The mapping or sequence that receives the snippet; the root by default. */
  parent?: YamlPath;
  /** Paths the snippet adds that may exist only once; any present disables it. */
  conflicts?: YamlPath[];
};

const statusSchema = `|-
  {
    "type": "string",
    "enum": ["\${3:draft}", "\${4:live}"],
    "x-attricat-status": {
      "version": 1,
      "options": [
        { "code": "\${3}", "label": "\${5:Draft}" },
        { "code": "\${4}", "label": "\${6:Live}", "tone": "success" }
      ],
      "transitions": [
        { "from": null, "to": "\${3}" },
        { "from": "\${3}", "to": "\${4}" }
      ]
    }
  }`;

const attribute = (rest: string): DefinitionSnippet['body'] =>
  `code: \${1:code}\nname: \${2:Name}\n${rest}`;

const attributeSnippet = (id: string, rest: string): DefinitionSnippet => ({
  id,
  group: 'attributes',
  body: attribute(rest),
  placement: snippetPlacements.item,
  parent: ['attributes'],
});

const viewSnippet = (id: string, view: string, body: string) => ({
  id,
  group: 'views',
  body,
  placement: snippetPlacements.entry,
  parent: ['views'],
  conflicts: [['views', view]],
});

const blueprintSnippets: DefinitionSnippet[] = [
  attributeSnippet('textAttribute', 'value_type: string'),
  attributeSnippet(
    'numberAttribute',
    'value_type: number\nvalue_schema: \'{"type":"number","minimum":${3:0}}\'',
  ),
  attributeSnippet(
    'choiceAttribute',
    'value_type: string\nvalue_schema: \'{"type":"string","enum":["${3:s}","${4:l}"],"x-attricat-enum-labels":{"${3}":"${5:Small}","${4}":"${6:Large}"}}\'',
  ),
  attributeSnippet(
    'statusAttribute',
    `value_type: string\nvalue_schema: ${statusSchema}`,
  ),
  attributeSnippet(
    'assigneeAttribute',
    'value_type: string\nvalue_schema: \'{"type":"string","x-attricat-principal":{"version":1,"kinds":["user","team"]}}\'',
  ),
  attributeSnippet(
    'relationshipAttribute',
    'value_type: relationship\ntarget_blueprint: ${3:blueprint}\ncardinality: ${4|one,many|}',
  ),
  attributeSnippet(
    'imageAttribute',
    'value_type: file\ncardinality: ${3|one,many|}\nallowed_mime_groups: [image]\nimage_only: true',
  ),
  attributeSnippet(
    'documentAttribute',
    'value_type: file\ncardinality: many\nallowed_extensions: [${3:pdf}]',
  ),
  viewSnippet(
    'detailView',
    'detail',
    'detail:\n  type: stack\n  children:\n    - { type: field, field: ${1:code} }',
  ),
  viewSnippet(
    'tableView',
    'table',
    'table:\n  type: table\n  columns:\n    - { field: ${1:code} }',
  ),
  viewSnippet(
    'explorerView',
    'explorer',
    'explorer:\n  type: explorer\n  browse_by: ${1:relationship}',
  ),
  {
    id: 'requiredAttributes',
    group: 'validation',
    body: 'record_schema: \'{"type":"object","required":["${1:code}"]}\'',
    placement: snippetPlacements.key,
    conflicts: [['record_schema']],
  },
  {
    id: 'uniqueKey',
    group: 'validation',
    body: 'code: ${1:code}\nattributes: [${2:code}]',
    placement: snippetPlacements.item,
    parent: ['unique_keys'],
  },
  {
    id: 'include',
    group: 'structure',
    body: 'alias: ${1:alias}\ncode: ${2:mixin}\nversion: ${3:1}',
    placement: snippetPlacements.item,
    parent: ['includes'],
  },
];

/** A reusable attribute has one value schema, so every one conflicts. */
const valueSchema = (id: string, body: string): DefinitionSnippet => ({
  id,
  group: 'values',
  body: `value_schema:\n${body}`,
  placement: snippetPlacements.entry,
  conflicts: [['value_schema']],
});

const reusableAttributeSnippets: DefinitionSnippet[] = [
  valueSchema(
    'allowedValues',
    '  type: string\n  enum: [${1:s}, ${2:l}]\n  x-attricat-enum-labels: { ${1}: ${3:Small}, ${2}: ${4:Large} }',
  ),
  valueSchema(
    'numberRange',
    '  type: number\n  minimum: ${1:0}\n  maximum: ${2:100}',
  ),
  valueSchema(
    'textPattern',
    "  type: string\n  pattern: '${1:^[A-Z0-9-]+\\$}'",
  ),
  valueSchema(
    'status',
    '  type: string\n  enum: [${1:draft}, ${2:live}]\n  x-attricat-status:\n    version: 1\n    options:\n      - { code: ${1}, label: ${3:Draft} }\n      - { code: ${2}, label: ${4:Live}, tone: success }',
  ),
  valueSchema(
    'assignee',
    '  type: string\n  x-attricat-principal: { version: 1, kinds: [user, team] }',
  ),
  {
    id: 'defaultValue',
    group: 'values',
    body: 'default_value: ${1:value}',
    placement: snippetPlacements.key,
    conflicts: [['default_value']],
  },
  {
    id: 'searchable',
    group: 'behavior',
    body: 'searchable: true',
    placement: snippetPlacements.key,
    conflicts: [['searchable']],
  },
  {
    id: 'facetable',
    group: 'behavior',
    body: 'facetable: true',
    placement: snippetPlacements.key,
    conflicts: [['facetable']],
  },
  {
    id: 'defaultContextOnly',
    group: 'behavior',
    body: 'context_editable: default',
    placement: snippetPlacements.key,
    conflicts: [['context_editable']],
  },
  {
    id: 'hidden',
    group: 'behavior',
    body: 'tags: [${1|hidden,hidden:form,hidden:detail,hidden:explorer,hidden:metadata|}]',
    placement: snippetPlacements.key,
    conflicts: [['tags']],
  },
  {
    id: 'relationshipTarget',
    group: 'relationshipsAndFiles',
    body: 'target_blueprint_code: ${1:blueprint}\ncardinality: ${2|one,many|}',
    placement: snippetPlacements.key,
    conflicts: [['target_blueprint_code'], ['cardinality']],
  },
  {
    id: 'filePolicy',
    group: 'relationshipsAndFiles',
    body: 'file_policy:\n  cardinality: ${1|one,many|}\n  ordered: false\n  allowed_mime_groups: [${2:image}]\n  allowed_extensions: []\n  purposes: []\n  image_only: ${3|true,false|}',
    placement: snippetPlacements.entry,
    conflicts: [['file_policy']],
  },
];

/** A rule has exactly one predicate, so every predicate conflicts. */
const predicate = (id: string, body: string): DefinitionSnippet => ({
  id,
  group: 'predicates',
  body: `predicate:\n${body
    .split('\n')
    .map((line) => `  ${line}`)
    .join('\n')}`,
  placement: snippetPlacements.entry,
  conflicts: [['predicate']],
});

const trigger = (id: string, body: string): DefinitionSnippet => ({
  id,
  group: 'triggers',
  body,
  placement: snippetPlacements.item,
  parent: ['triggers'],
});

const ruleSnippets: DefinitionSnippet[] = [
  trigger('manualTrigger', 'type: manual'),
  trigger(
    'scheduleTrigger',
    "type: schedule\ncron: '${1:0 0 3 * * *}'\ntimezone: UTC",
  ),
  trigger(
    'eventTrigger',
    'type: event\nevent_type: ${1|record.updated.v1,record.created.v1,record.migrated.v1,attribute_value.changed.v1,attribute_value.restored.v1,relationship.changed.v1|}',
  ),
  predicate(
    'requiredPredicate',
    'type: required\nattribute_code: ${1:attribute}',
  ),
  predicate(
    'oneOfPredicate',
    'type: one_of\nattribute_code: ${1:attribute}\nvalues: [${2:value}]',
  ),
  predicate(
    'comparePredicate',
    'type: compare\nattribute_code: ${1:attribute}\nop: ${2|gte,gt,lte,lt,eq,ne|}\nvalue: ${3:0}',
  ),
  predicate(
    'relativeDatePredicate',
    'type: relative_date\nattribute_code: ${1:attribute}\nop: ${2|gt,gte,lt,lte|}\noffset_days: ${3:30}',
  ),
  predicate(
    'stalePredicate',
    'type: stale\nattribute_code: ${1:attribute}\nmax_age_seconds: ${2:7776000}',
  ),
  predicate(
    'uniquePredicate',
    'type: unique\nattribute_codes: [${1:attribute}]',
  ),
  {
    id: 'enforceOnSave',
    group: 'enforcement',
    body: 'enforcement:\n  on_save: true',
    placement: snippetPlacements.entry,
    conflicts: [['enforcement']],
  },
  {
    id: 'enforceTransition',
    group: 'enforcement',
    body: 'attribute_code: ${1:status}\nto: ${2:released}',
    placement: snippetPlacements.item,
    parent: ['enforcement', 'transitions'],
  },
];

export const definitionSnippets: Record<DefinitionKind, DefinitionSnippet[]> = {
  [definitionKinds.blueprint]: blueprintSnippets,
  [definitionKinds.reusableAttribute]: reusableAttributeSnippets,
  [definitionKinds.rule]: ruleSnippets,
  // Workflows are edited without the Insert menu.
  [definitionKinds.workflow]: [],
};

/**
 * Whether the definition, scanned with `scanYaml`, already has something the
 * snippet would add.
 */
export const snippetIsPresent = (
  outline: YamlOutline,
  snippet: DefinitionSnippet,
) =>
  (snippet.conflicts ?? []).some((path) =>
    outline.keysIn(path.slice(0, -1)).has(String(path.at(-1))),
  );

const indentUnit = '  ';

const samePath = (left: YamlPath, right: YamlPath) =>
  left.length === right.length &&
  left.every((segment, index) => segment === right[index]);

const lineStart = (text: string, offset: number) =>
  text.lastIndexOf('\n', offset - 1) + 1;

const lineEnd = (text: string, offset: number) => {
  const end = text.indexOf('\n', offset);
  return end < 0 ? text.length : end;
};

const blockAt = (outline: YamlOutline, path: YamlPath) =>
  outline.collections.find(
    (collection) => !collection.flow && samePath(collection.path, path),
  );

/** The column of a block collection's children: its keys or its dashes. */
const childIndent = (
  text: string,
  outline: YamlOutline,
  block: YamlCollection,
) => {
  const first = outline.entries.find(
    ({ path }) =>
      path.length === block.path.length + 1 &&
      samePath(path.slice(0, -1), block.path),
  );
  if (!first) return 0;
  const column = first.range.start - lineStart(text, first.range.start);
  // A sequence item's content follows its `- `.
  return block.kind === 'seq' ? Math.max(column - 2, 0) : column;
};

/** The last root `key: value` written on one line, which keys follow. */
const lastRootKey = (text: string, outline: YamlOutline) =>
  outline.entries
    .filter(
      ({ path, valueRange }) =>
        path.length === 1 &&
        valueRange !== undefined &&
        !text.slice(valueRange.start, valueRange.end).includes('\n'),
    )
    .at(-1);

/**
 * Where a snippet goes and the text to insert there. Root keys follow the
 * last single-line root key. Blocks and items are appended to the end of
 * their parent, which is created when missing; root blocks are separated by
 * a blank line.
 */
export const snippetEdit = (text: string, snippet: DefinitionSnippet) => {
  const outline = scanYaml(text);
  const lines = snippet.body.split('\n');
  if (snippet.placement === snippetPlacements.key) {
    const anchor = lastRootKey(text, outline);
    if (anchor)
      return {
        insertText: `\n${lines.join('\n')}`,
        offset: lineEnd(text, anchor.range.start),
      };
  }
  const parent = snippet.parent ?? [];
  let depth = parent.length;
  let host = depth ? blockAt(outline, parent) : undefined;
  while (depth > 0 && !host) host = blockAt(outline, parent.slice(0, --depth));

  let body =
    snippet.placement === snippetPlacements.item
      ? lines.map((line, index) => (index ? indentUnit : '- ') + line)
      : lines;
  for (const key of parent.slice(depth).reverse())
    body = [
      `${yamlKey(String(key))}:`,
      ...body.map((line) => indentUnit + line),
    ];

  if (!host) {
    const trimmed = text.trimEnd();
    const separator =
      trimmed.length === 0
        ? ''
        : snippet.placement === snippetPlacements.key
          ? '\n'
          : '\n\n';
    return {
      insertText: `${separator}${body.join('\n')}\n`,
      offset: trimmed.length,
      replaceEnd: text.length,
    };
  }
  let end = host.range.end;
  while (end > host.range.start && /\s/.test(text[end - 1]!)) end--;
  const indent = ' '.repeat(childIndent(text, outline, host));
  // Children spaced by blank lines get one before the new child too.
  const spacing = /\n[ \t]*\n/.test(text.slice(host.range.start, end))
    ? '\n'
    : '';
  return {
    insertText: `\n${spacing}${body.map((line) => (line ? indent + line : line)).join('\n')}`,
    offset: end,
  };
};
