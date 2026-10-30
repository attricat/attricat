import { describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { definitionKinds, type DefinitionKind } from './constants';
import { definitionCompletions } from './definitionCompletions';
import type { DefinitionReferences } from './definitionReferences';

const cursorMarker = '|';

const references: DefinitionReferences = {
  blueprintAttributes: vi.fn(async (code: string) =>
    code === 'category'
      ? [
          { code: 'name', valueType: 'string' },
          {
            code: 'parent',
            targetBlueprint: 'category',
            valueType: 'relationship',
          },
        ]
      : code === 'seo'
        ? [
            { code: 'meta_title', valueType: 'string' },
            { code: 'meta_description', valueType: 'string' },
          ]
        : [],
  ),
  blueprints: vi.fn(async () => [
    {
      code: 'category',
      id: 'category-id',
      kind: 'record',
      name: 'Category',
      version: 3,
    },
    { code: 'seo', id: 'seo-id', kind: 'mixin', name: 'SEO', version: 2 },
  ]),
  contexts: vi.fn(async () => ['default', 'pl']),
  roles: vi.fn(async () => ['attricat_manager']),
};

const complete = async (
  source: string,
  kind: DefinitionKind = definitionKinds.blueprint,
) => {
  const offset = source.indexOf(cursorMarker);
  const text = source.replace(cursorMarker, '');
  const suggestions = await definitionCompletions({
    kind,
    offset,
    references,
    text,
  });
  return {
    labels: suggestions.map((suggestion) => suggestion.label),
    suggestions,
    text,
  };
};

const header = `format_version: 1
code: product
name: Product
kind: record
`;

describe('definitionCompletions', () => {
  it('suggests missing top-level keys and blocks', async () => {
    const { labels, suggestions } = await complete(`${header}|`);
    expect(labels).toEqual(
      expect.arrayContaining([
        'attributes',
        'includes',
        'views',
        'record_schema',
      ]),
    );
    expect(labels).not.toContain('code');
    expect(
      suggestions.find((suggestion) => suggestion.label === 'attributes')
        ?.insertText,
    ).toBe('attributes:\n  - code: ${1}');
    expect(
      suggestions.find((suggestion) => suggestion.label === 'views')
        ?.insertText,
    ).toBe(
      'views:\n  ${1|dropdown_option,page_title,detail,table,explorer,extension_layout|}:\n    type: ${2|dropdown_option,page_title,table,explorer,stack,grid,section,tabs,accordion,extension_layout|}',
    );
  });

  it('offers attribute keys filtered by value type', async () => {
    const file = await complete(
      `${header}attributes:\n  - code: images\n    value_type: file\n    |`,
    );
    expect(file.labels).toEqual(
      expect.arrayContaining([
        'allowed_mime_groups',
        'image_only',
        'cardinality',
      ]),
    );
    expect(file.labels).not.toContain('target_blueprint');
    const text = await complete(
      `${header}attributes:\n  - code: name\n    value_type: string\n    |`,
    );
    expect(text.labels).not.toContain('image_only');
    expect(text.labels).toContain('default_value');
  });

  it('offers enum values with snippet choices and plain values', async () => {
    const key = await complete(`${header}attributes:\n  - code: a\n    value|`);
    expect(
      key.suggestions.find((suggestion) => suggestion.label === 'value_type')
        ?.insertText,
    ).toBe(
      'value_type: ${1|string,number,integer,boolean,date,datetime,time,json,relationship,file|}',
    );
    const value = await complete("kind: '|'");
    expect(value.labels).toEqual(['record', 'mixin']);
    expect(value.suggestions[1].documentation).toBe(
      'Provides attributes for other blueprints to include.',
    );
    const unquoted = await complete('kind: |');
    expect(unquoted.suggestions[0].insertText).toBe('record');
  });

  it('suggests view block variants with their required keys', async () => {
    const { suggestions } = await complete(
      `${header}views:\n  detail:\n    type: stack\n    |`,
    );
    const field = suggestions.find(
      (suggestion) => suggestion.label === 'children: field',
    );
    expect(field?.insertText).toBe(
      'children:\n  - type: field\n    field: ${1}',
    );
    const inline = await complete(
      `${header}views:\n  detail:\n    type: stack\n    children: [|]`,
    );
    expect(
      inline.suggestions.find(
        (suggestion) => suggestion.label === '{ type: heading }',
      )?.insertText,
    ).toBe('{ type: heading, text: ${1} }');
  });

  it('asks for a block type before other keys', async () => {
    const { labels, suggestions } = await complete(
      `${header}views:\n  detail:\n    type: stack\n    children:\n      - |`,
    );
    expect(labels).toContain('type: field');
    expect(labels).not.toContain('field');
    // Keys after the first align with it, past the `- `.
    expect(
      suggestions.find((suggestion) => suggestion.label === 'type: field')
        ?.insertText,
    ).toBe('type: field\n  field: ${1}');
  });

  it('completes attribute references from the document', async () => {
    const { suggestions } = await complete(
      `${header}views:\n  dropdown_option:\n    type: dropdown_option\n    fields: [|]\nattributes:\n  - code: name\n    value_type: string\n  - code: category\n    value_type: relationship\n    target_blueprint: category\n`,
    );
    expect(suggestions.map(({ detail, label }) => ({ detail, label }))).toEqual(
      [
        { detail: 'string', label: 'name' },
        { detail: 'Relationship to category', label: 'category' },
      ],
    );
  });

  it('completes attribute references in block sequence items', async () => {
    const { labels } = await complete(
      `${header}views:\n  dropdown_option:\n    type: dropdown_option\n    fields:\n      - |\nattributes:\n  - code: name\n    value_type: string\n`,
    );
    expect(labels).toEqual(['name']);
  });

  it('completes the explorer browse_by attribute', async () => {
    const { labels } = await complete(
      `${header}views:\n  explorer:\n    type: explorer\n    browse_by: |\nattributes:\n  - code: category\n    value_type: relationship\n    target_blueprint: category\n`,
    );
    expect(labels).toContain('category');
  });

  it('follows relationships only in browse_by paths', async () => {
    const attributes = `attributes:\n  - code: title\n    value_type: string\n  - code: category\n    value_type: relationship\n    target_blueprint: category\n`;
    const view = `${header}views:\n  explorer:\n    type: explorer\n`;
    const first = await complete(`${view}    browse_by: |\n${attributes}`);
    expect(first.labels).toEqual(['category']);
    expect(first.suggestions[0].insertText).toBe('category.$0');
    const second = await complete(
      `${view}    browse_by: 'category.|'\n${attributes}`,
    );
    expect(second.labels).toEqual(['parent']);
    // A third hop is the last one a browse path follows.
    const third = await complete(
      `${view}    browse_by: 'category.parent.|'\n${attributes}`,
    );
    expect(third.suggestions[0]).toMatchObject({ insertText: 'parent' });
  });

  it('follows relationship hops in table column paths', async () => {
    const attributes = `attributes:\n  - code: category\n    value_type: relationship\n    target_blueprint: category\n`;
    const table = `${header}views:\n  table:\n    type: table\n    columns:\n`;
    const first = await complete(`${table}      - field: cat|\n${attributes}`);
    expect(first.suggestions[0]).toMatchObject({
      insertText: 'category.$0',
      triggerSuggest: true,
    });
    const second = await complete(
      `${table}      - field: 'category.pa|'\n${attributes}`,
    );
    expect(second.labels).toEqual(['name', 'parent']);
    expect(second.suggestions[1].insertText).toBe('parent.');
    expect(
      second.text.slice(
        second.suggestions[0].range.start,
        second.suggestions[0].range.end,
      ),
    ).toBe('pa');
  });

  it('completes include selections and mixin references', async () => {
    const includes = `includes:\n  - alias: seo\n    code: seo\n    version: 2\n`;
    const mixins = await complete(`${header}includes:\n  - code: |`);
    expect(mixins.labels).toEqual(['seo']);
    const version = await complete(
      `${header}includes:\n  - code: seo\n    version: |`,
    );
    expect(version.suggestions[0]).toMatchObject({ insertText: '2' });
    const alias = await complete(
      `${header}${includes}attributes:\n  - code: meta_title\n    from: |`,
    );
    expect(alias.suggestions[0]).toMatchObject({
      insertText: 'seo.$0',
      triggerSuggest: true,
    });
    const selection = await complete(
      `${header}${includes}attributes:\n  - code: meta_title\n    from: 'seo.|'`,
    );
    expect(selection.labels).toEqual(['meta_title', 'meta_description']);
    expect(selection.suggestions[0].sortText).toBe('0');
    expect(references.blueprintAttributes).toHaveBeenCalledWith('seo', 2);
  });

  it('suggests components that fit the block, view, and field', async () => {
    const attributes = `attributes:\n  - code: price\n    value_type: number\n`;
    const children = `${header}views:\n  detail:\n    type: stack\n    children:\n`;
    const detail = await complete(
      `${children}      - type: field\n        field: price\n        component: |\n${attributes}`,
    );
    expect(
      detail.suggestions.map((suggestion) => suggestion.insertText),
    ).toEqual(['{ id: attricat.field_display, version: 1 }']);
    const renderer = await complete(
      `${header}views:\n  table:\n    type: table\n    columns:\n      - field: photo\n        renderer: {id: |}\n${attributes}  - code: photo\n    value_type: file\n`,
    );
    expect(renderer.labels).toEqual(['attricat.table_image']);
  });

  it('suggests URL controls only for matching string placements', async () => {
    const attributes =
      'attributes:\n  - code: website\n    value_type: string\n';
    const detail = await complete(
      `${header}views:\n  detail:\n    type: stack\n    children:\n      - type: field\n        field: website\n        component: {id: |}\n${attributes}`,
    );
    expect(detail.labels).toContain('attricat.url_display');
    expect(detail.labels).not.toContain('attricat.url_edit');
    const table = await complete(
      `${header}views:\n  table:\n    type: table\n    columns:\n      - field: website\n        renderer: {id: |}\n${attributes}`,
    );
    expect(table.labels).toContain('attricat.url_display');
    expect(table.labels).not.toContain('attricat.url_edit');
  });

  it('completes roles, events, and tags', async () => {
    expect(
      (await complete(`${header}publication:\n  retain_on_edit_roles: [|]`))
        .labels,
    ).toEqual(['attricat_manager']);
    expect(
      (
        await complete(
          'format_version: 1\ntriggers:\n  - type: event\n    event_type: |',
          definitionKinds.rule,
        )
      ).labels,
    ).toContain('attribute_value.changed.v1');
    expect(
      (await complete(`${header}attributes:\n  - code: a\n    tags: [hid|]`))
        .labels,
    ).toContain('hidden:form');
  });

  it('uses the reusable attribute contract', async () => {
    const { labels } = await complete(
      'code: brand\nname: Brand\nvalue_type: relationship\n|',
      definitionKinds.reusableAttribute,
    );
    expect(labels).toEqual(
      expect.arrayContaining([
        'target_blueprint_code',
        'searchable',
        'facetable',
      ]),
    );
    expect(labels).not.toContain('file_policy');
    const values = await complete(
      'code: brand\nname: Brand\nvalue_type: |',
      definitionKinds.reusableAttribute,
    );
    expect(values.labels).not.toContain('json');
  });

  it('completes rule predicates from the rule blueprint’s attributes', async () => {
    const rule = `format_version: 1
code: title-required
name: Title required
severity: error
triggers:
  - type: manual
predicate:
`;
    const types = await complete(`${rule}  type: |`, definitionKinds.rule);
    expect(types.labels).toEqual(
      expect.arrayContaining(['required', 'one_of', 'stale', 'unique']),
    );
    const offset = `${rule}  type: required\n  attribute_code: |`;
    const suggestions = await definitionCompletions({
      kind: definitionKinds.rule,
      offset: offset.indexOf(cursorMarker),
      references: {
        ...references,
        scopeAttributes: [
          { code: 'title', valueType: 'string' },
          {
            code: 'brand',
            targetBlueprint: 'brand',
            valueType: 'relationship',
          },
        ],
      },
      text: offset.replace(cursorMarker, ''),
    });
    expect(suggestions.map(({ detail, label }) => ({ detail, label }))).toEqual(
      [
        { detail: 'string', label: 'title' },
        { detail: 'Relationship to brand', label: 'brand' },
      ],
    );
  });

  describe('workflows', () => {
    const workflow = (source: string) =>
      complete(
        `format_version: 2\ncode: w\nname: W\n\n${source}`,
        definitionKinds.workflow,
      );

    it('offers event trigger keys and the typed trigger variants', async () => {
      const { labels } = await workflow('triggers:\n  - |');
      // Keys follow the contract's declaration order.
      expect(labels).toEqual([
        'event_type',
        'envelope',
        'facts',
        'attributes',
        'blueprints',
        'change_kinds',
        'tags_added',
        'tags_removed',
        'type: manual',
        'type: schedule',
        'type: extension_event',
      ]);
    });

    it('completes event types of untyped triggers', async () => {
      const { labels } = await workflow('triggers:\n  - event_type: |');
      expect(labels).toContain('record.created.v1');
      expect(labels).not.toContain('plugin.example.v1');
    });

    it('completes the keys of a typed trigger', async () => {
      const { labels } = await workflow('triggers:\n  - type: schedule\n    |');
      expect(labels).toEqual(['cron', 'timezone', 'target_record_id']);
    });
  });
});
