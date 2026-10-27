import { definitionKinds, type DefinitionKind } from './constants';
import { scanToml, type TomlOutline, type TomlPath } from './tomlOutline';

const snippetPlacements = {
  /** A top-level key, inserted before the first table header. */
  key: 'key',
  /** A table or array-of-tables, appended to the end of the definition. */
  section: 'section',
} as const;
export type SnippetPlacement =
  (typeof snippetPlacements)[keyof typeof snippetPlacements];

export type DefinitionSnippet = {
  /** Translation key suffix under `definitionEditor.snippets`. */
  id: string;
  /** Translation key suffix under `definitionEditor.snippetGroups`. */
  group: string;
  /** Monaco snippet syntax: `${1:default}` tab stops, `${1|a,b|}` choices. */
  body: string;
  placement: SnippetPlacement;
  /** Paths the snippet adds that may exist only once; any present disables it. */
  conflicts?: TomlPath[];
};

const statusSchema = `'''{
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
}'''`;

const attribute = (rest: string) =>
  `[[attributes]]\ncode = "\${1:code}"\nname = "\${2:Name}"\n${rest}`;

const blueprintSnippets: DefinitionSnippet[] = [
  {
    id: 'textAttribute',
    group: 'attributes',
    body: attribute('value_type = "string"'),
    placement: snippetPlacements.section,
  },
  {
    id: 'numberAttribute',
    group: 'attributes',
    body: attribute(
      'value_type = "number"\nvalue_schema = \'{"type":"number","minimum":${3:0}}\'',
    ),
    placement: snippetPlacements.section,
  },
  {
    id: 'choiceAttribute',
    group: 'attributes',
    body: attribute(
      'value_type = "string"\nvalue_schema = \'{"type":"string","enum":["${3:s}","${4:l}"],"x-attricat-enum-labels":{"${3}":"${5:Small}","${4}":"${6:Large}"}}\'',
    ),
    placement: snippetPlacements.section,
  },
  {
    id: 'statusAttribute',
    group: 'attributes',
    body: attribute(`value_type = "string"\nvalue_schema = ${statusSchema}`),
    placement: snippetPlacements.section,
  },
  {
    id: 'assigneeAttribute',
    group: 'attributes',
    body: attribute(
      'value_type = "string"\nvalue_schema = \'{"type":"string","x-attricat-principal":{"version":1,"kinds":["user","team"]}}\'',
    ),
    placement: snippetPlacements.section,
  },
  {
    id: 'relationshipAttribute',
    group: 'attributes',
    body: attribute(
      'value_type = "relationship"\ntarget_blueprint = "${3:blueprint}"\ncardinality = "${4|one,many|}"',
    ),
    placement: snippetPlacements.section,
  },
  {
    id: 'imageAttribute',
    group: 'attributes',
    body: attribute(
      'value_type = "file"\ncardinality = "${3|one,many|}"\nallowed_mime_groups = ["image"]\nimage_only = true',
    ),
    placement: snippetPlacements.section,
  },
  {
    id: 'documentAttribute',
    group: 'attributes',
    body: attribute(
      'value_type = "file"\ncardinality = "many"\nallowed_extensions = ["${3:pdf}"]',
    ),
    placement: snippetPlacements.section,
  },
  {
    id: 'detailView',
    group: 'views',
    body: '[views.detail]\ntype = "stack"\nchildren = [\n  { type = "field", field = "${1:code}" },\n]',
    placement: snippetPlacements.section,
    conflicts: [['views', 'detail']],
  },
  {
    id: 'tableView',
    group: 'views',
    body: '[views.table]\ntype = "table"\ncolumns = [{ field = "${1:code}" }]',
    placement: snippetPlacements.section,
    conflicts: [['views', 'table']],
  },
  {
    id: 'explorerView',
    group: 'views',
    body: '[views.explorer]\ntype = "explorer"\nbrowse_by = "${1:relationship}"',
    placement: snippetPlacements.section,
    conflicts: [['views', 'explorer']],
  },
  {
    id: 'requiredAttributes',
    group: 'validation',
    body: 'record_schema = \'{"type":"object","required":["${1:code}"]}\'',
    placement: snippetPlacements.key,
    conflicts: [['record_schema']],
  },
  {
    id: 'uniqueKey',
    group: 'validation',
    body: '[[unique_keys]]\ncode = "${1:code}"\nattributes = ["${2:code}"]',
    placement: snippetPlacements.section,
  },
  {
    id: 'rule',
    group: 'validation',
    body: '[[rules]]\ncode = "${1:rule-code}"\nname = "${2:Rule name}"\nseverity = "${3|warning,error|}"\ntriggers = [{ type = "schedule", cron = "0 0 3 * * 1", timezone = "UTC" }]\npredicate = { type = "stale", attribute_code = "${4:code}", max_age_seconds = ${5:7776000} }',
    placement: snippetPlacements.section,
  },
  {
    id: 'include',
    group: 'structure',
    body: '[[includes]]\nalias = "${1:alias}"\ncode = "${2:mixin}"\nversion = ${3:1}',
    placement: snippetPlacements.section,
  },
];

const reusableAttributeSnippets: DefinitionSnippet[] = [
  {
    id: 'allowedValues',
    group: 'values',
    body: 'value_schema = { type = "string", enum = ["${1:s}", "${2:l}"], "x-attricat-enum-labels" = { "${1}" = "${3:Small}", "${2}" = "${4:Large}" } }',
    placement: snippetPlacements.key,
    conflicts: [['value_schema']],
  },
  {
    id: 'numberRange',
    group: 'values',
    body: 'value_schema = { type = "number", minimum = ${1:0}, maximum = ${2:100} }',
    placement: snippetPlacements.key,
    conflicts: [['value_schema']],
  },
  {
    id: 'textPattern',
    group: 'values',
    body: 'value_schema = { type = "string", pattern = "${1:^[A-Z0-9-]+\\$}" }',
    placement: snippetPlacements.key,
    conflicts: [['value_schema']],
  },
  {
    id: 'status',
    group: 'values',
    body: '[value_schema]\ntype = "string"\nenum = ["${1:draft}", "${2:live}"]\n\n[value_schema."x-attricat-status"]\nversion = 1\noptions = [\n  { code = "${1}", label = "${3:Draft}" },\n  { code = "${2}", label = "${4:Live}", tone = "success" },\n]',
    placement: snippetPlacements.section,
    conflicts: [['value_schema']],
  },
  {
    id: 'assignee',
    group: 'values',
    body: 'value_schema = { type = "string", "x-attricat-principal" = { version = 1, kinds = ["user", "team"] } }',
    placement: snippetPlacements.key,
    conflicts: [['value_schema']],
  },
  {
    id: 'defaultValue',
    group: 'values',
    body: 'default_value = "${1:value}"',
    placement: snippetPlacements.key,
    conflicts: [['default_value']],
  },
  {
    id: 'searchable',
    group: 'behavior',
    body: 'searchable = true',
    placement: snippetPlacements.key,
    conflicts: [['searchable']],
  },
  {
    id: 'facetable',
    group: 'behavior',
    body: 'facetable = true',
    placement: snippetPlacements.key,
    conflicts: [['facetable']],
  },
  {
    id: 'defaultContextOnly',
    group: 'behavior',
    body: 'context_editable = "default"',
    placement: snippetPlacements.key,
    conflicts: [['context_editable']],
  },
  {
    id: 'hidden',
    group: 'behavior',
    body: 'tags = ["${1|hidden,hidden:form,hidden:detail,hidden:explorer,hidden:metadata|}"]',
    placement: snippetPlacements.key,
    conflicts: [['tags']],
  },
  {
    id: 'relationshipTarget',
    group: 'relationshipsAndFiles',
    body: 'target_blueprint_code = "${1:blueprint}"\ncardinality = "${2|one,many|}"',
    placement: snippetPlacements.key,
    conflicts: [['target_blueprint_code'], ['cardinality']],
  },
  {
    id: 'filePolicy',
    group: 'relationshipsAndFiles',
    body: '[file_policy]\ncardinality = "${1|one,many|}"\nordered = false\nallowed_mime_groups = ["${2:image}"]\nallowed_extensions = []\npurposes = []\nimage_only = ${3|true,false|}',
    placement: snippetPlacements.section,
    conflicts: [['file_policy']],
  },
];

/** A rule has exactly one predicate, so every predicate conflicts. */
const predicate = (id: string, body: string): DefinitionSnippet => ({
  id,
  group: 'predicates',
  body: `[predicate]\n${body}`,
  placement: snippetPlacements.section,
  conflicts: [['predicate']],
});

const ruleSnippets: DefinitionSnippet[] = [
  {
    id: 'manualTrigger',
    group: 'triggers',
    body: '[[triggers]]\ntype = "manual"',
    placement: snippetPlacements.section,
  },
  {
    id: 'scheduleTrigger',
    group: 'triggers',
    body: '[[triggers]]\ntype = "schedule"\ncron = "${1:0 0 3 * * *}"\ntimezone = "UTC"',
    placement: snippetPlacements.section,
  },
  {
    id: 'eventTrigger',
    group: 'triggers',
    body: '[[triggers]]\ntype = "event"\nevent_type = "${1|record.updated.v1,record.created.v1,record.migrated.v1,attribute_value.changed.v1,attribute_value.restored.v1,relationship.changed.v1|}"',
    placement: snippetPlacements.section,
  },
  predicate(
    'requiredPredicate',
    'type = "required"\nattribute_code = "${1:attribute}"',
  ),
  predicate(
    'oneOfPredicate',
    'type = "one_of"\nattribute_code = "${1:attribute}"\nvalues = ["${2:value}"]',
  ),
  predicate(
    'comparePredicate',
    'type = "compare"\nattribute_code = "${1:attribute}"\nop = "${2|gte,gt,lte,lt,eq,ne|}"\nvalue = ${3:0}',
  ),
  predicate(
    'relativeDatePredicate',
    'type = "relative_date"\nattribute_code = "${1:attribute}"\nop = "${2|gt,gte,lt,lte|}"\noffset_days = ${3:30}',
  ),
  predicate(
    'stalePredicate',
    'type = "stale"\nattribute_code = "${1:attribute}"\nmax_age_seconds = ${2:7776000}',
  ),
  predicate(
    'uniquePredicate',
    'type = "unique"\nattribute_codes = ["${1:attribute}"]',
  ),
  {
    id: 'enforceOnSave',
    group: 'enforcement',
    body: '[enforcement]\non_save = true',
    placement: snippetPlacements.section,
    conflicts: [['enforcement']],
  },
  {
    id: 'enforceTransition',
    group: 'enforcement',
    body: '[[enforcement.transitions]]\nattribute_code = "${1:status}"\nto = "${2:released}"',
    placement: snippetPlacements.section,
  },
];

export const definitionSnippets: Record<DefinitionKind, DefinitionSnippet[]> = {
  [definitionKinds.blueprint]: blueprintSnippets,
  [definitionKinds.reusableAttribute]: reusableAttributeSnippets,
  [definitionKinds.rule]: ruleSnippets,
};

/**
 * Whether the definition, scanned with `scanToml`, already has something the
 * snippet would add.
 */
export const snippetIsPresent = (
  outline: TomlOutline,
  snippet: DefinitionSnippet,
) =>
  (snippet.conflicts ?? []).some((path) =>
    outline.keysIn(path.slice(0, -1)).has(String(path.at(-1))),
  );

/**
 * Where a snippet goes and the text to insert there. Keys go after the last
 * top-level key, since everything after the first header belongs to a table;
 * sections go at the end, separated by a blank line.
 */
export const snippetEdit = (text: string, snippet: DefinitionSnippet) => {
  const firstHeader = scanToml(text).headers[0];
  if (snippet.placement === snippetPlacements.key && firstHeader) {
    const offset = text.lastIndexOf('\n', firstHeader.range.start - 1) + 1;
    return { insertText: `${snippet.body}\n\n`, offset };
  }
  const trimmed = text.trimEnd();
  const separator =
    trimmed.length === 0
      ? ''
      : snippet.placement === snippetPlacements.key
        ? '\n'
        : '\n\n';
  return {
    insertText: `${separator}${snippet.body}\n`,
    offset: trimmed.length,
    replaceEnd: text.length,
  };
};
