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
    { code: 'category', kind: 'entity', name: 'Category', version: 3 },
    { code: 'seo', kind: 'mixin', name: 'SEO', version: 2 },
  ]),
  contexts: vi.fn(async () => ['default', 'pl']),
  roles: vi.fn(async () => ['catalog_manager']),
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

const header = `format_version = 1
code = "product"
name = "Product"
kind = "entity"
`;

describe('definitionCompletions', () => {
  it('suggests missing top-level keys and sections', async () => {
    const { labels, suggestions } = await complete(`${header}|`);
    expect(labels).toEqual(
      expect.arrayContaining([
        '[[attributes]]',
        '[[includes]]',
        '[[rules]]',
        '[views.…]',
        'entity_schema',
      ]),
    );
    expect(labels).not.toContain('code');
    expect(
      suggestions.find((suggestion) => suggestion.label === '[[attributes]]')
        ?.insertText,
    ).toBe('[[attributes]]\ncode = "${1}"');
    expect(
      suggestions.find((suggestion) => suggestion.label === '[views.…]')
        ?.insertText,
    ).toBe(
      '[views.${1|dropdown_option,detail,edit,table,extension_layout|}]\ntype = "${2|dropdown_option,table,stack,grid,section,tabs,accordion,extension_layout|}"',
    );
  });

  it('offers attribute keys filtered by value type', async () => {
    const file = await complete(
      `${header}[[attributes]]\ncode = "images"\nvalue_type = "file"\n|`,
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
      `${header}[[attributes]]\ncode = "name"\nvalue_type = "string"\n|`,
    );
    expect(text.labels).not.toContain('image_only');
    expect(text.labels).toContain('default_value');
  });

  it('offers enum values with snippet choices and plain values', async () => {
    const key = await complete(`${header}[[attributes]]\ncode = "a"\nvalue|`);
    expect(
      key.suggestions.find((suggestion) => suggestion.label === 'value_type')
        ?.insertText,
    ).toBe(
      'value_type = "${1|string,number,integer,boolean,date,datetime,time,json,relationship,file|}"',
    );
    const value = await complete('kind = "|"');
    expect(value.labels).toEqual(['entity', 'mixin']);
    expect(value.suggestions[1].documentation).toBe(
      'Provides attributes for other blueprints to include.',
    );
    const unquoted = await complete('kind = |');
    expect(unquoted.suggestions[0].insertText).toBe('"entity"');
  });

  it('suggests view block variants with their required keys', async () => {
    const { suggestions } = await complete(
      `${header}[views.detail]\ntype = "stack"\n\n|`,
    );
    const field = suggestions.find(
      (suggestion) => suggestion.label === '[[views.detail.children]] field',
    );
    expect(field?.insertText).toBe(
      '[[views.detail.children]]\ntype = "field"\nfield = "${1}"',
    );
    const inline = await complete(
      `${header}[views.detail]\ntype = "stack"\nchildren = [|]`,
    );
    expect(
      inline.suggestions.find(
        (suggestion) => suggestion.label === '{ type = "heading" }',
      )?.insertText,
    ).toBe('{ type = "heading", text = "${1}" }');
  });

  it('asks for a block type before other keys', async () => {
    const { labels } = await complete(`${header}[[views.detail.children]]\n|`);
    expect(labels).toContain('type = "field"');
    expect(labels).not.toContain('field');
  });

  it('completes attribute references from the document', async () => {
    const { suggestions } = await complete(
      `${header}[views.dropdown_option]\ntype = "dropdown_option"\nfields = ["|"]\n\n[[attributes]]\ncode = "name"\nvalue_type = "string"\n\n[[attributes]]\ncode = "category"\nvalue_type = "relationship"\ntarget_blueprint = "category"\n`,
    );
    expect(suggestions.map(({ detail, label }) => ({ detail, label }))).toEqual(
      [
        { detail: 'string', label: 'name' },
        { detail: 'Relationship to category', label: 'category' },
      ],
    );
  });

  it('follows relationship hops in table column paths', async () => {
    const attributes = `\n[[attributes]]\ncode = "category"\nvalue_type = "relationship"\ntarget_blueprint = "category"\n`;
    const first = await complete(
      `${header}[[views.table.columns]]\nfield = "cat|"\n${attributes}`,
    );
    expect(first.suggestions[0]).toMatchObject({
      insertText: 'category.',
      triggerSuggest: true,
    });
    const second = await complete(
      `${header}[[views.table.columns]]\nfield = "category.pa|"\n${attributes}`,
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
    const includes = `[[includes]]\nalias = "seo"\ncode = "seo"\nversion = 2\n`;
    const mixins = await complete(`${header}[[includes]]\ncode = "|"`);
    expect(mixins.labels).toEqual(['seo']);
    const version = await complete(
      `${header}[[includes]]\ncode = "seo"\nversion = |`,
    );
    expect(version.suggestions[0]).toMatchObject({ insertText: '2' });
    const alias = await complete(
      `${header}${includes}\n[[attributes]]\ncode = "meta_title"\nfrom = "|"`,
    );
    expect(alias.suggestions[0]).toMatchObject({
      insertText: 'seo.',
      triggerSuggest: true,
    });
    const selection = await complete(
      `${header}${includes}\n[[attributes]]\ncode = "meta_title"\nfrom = "seo.|"`,
    );
    expect(selection.labels).toEqual(['meta_title', 'meta_description']);
    expect(selection.suggestions[0].sortText).toBe('0');
    expect(references.blueprintAttributes).toHaveBeenCalledWith('seo', 2);
  });

  it('suggests components that fit the block, view, and field', async () => {
    const attributes = `\n[[attributes]]\ncode = "price"\nvalue_type = "number"\n`;
    const edit = await complete(
      `${header}[[views.edit.children]]\ntype = "field"\nfield = "price"\ncomponent = |\n${attributes}`,
    );
    expect(edit.suggestions.map((suggestion) => suggestion.insertText)).toEqual(
      ['{ id = "catalog.field_edit", version = 1 }'],
    );
    const renderer = await complete(
      `${header}[[views.table.columns]]\nfield = "photo"\nrenderer = { id = "|" }\n${attributes}\n[[attributes]]\ncode = "photo"\nvalue_type = "file"\n`,
    );
    expect(renderer.labels).toEqual(['catalog.table_image']);
  });

  it('suggests URL controls only for matching string placements', async () => {
    const attributes =
      '\n[[attributes]]\ncode = "website"\nvalue_type = "string"\n';
    const edit = await complete(
      `${header}[[views.edit.children]]\ntype = "field"\nfield = "website"\ncomponent = { id = "|" }\n${attributes}`,
    );
    expect(edit.labels).toContain('catalog.url_edit');
    expect(edit.labels).not.toContain('catalog.url_display');
    const table = await complete(
      `${header}[[views.table.columns]]\nfield = "website"\nrenderer = { id = "|" }\n${attributes}`,
    );
    expect(table.labels).toContain('catalog.url_display');
    expect(table.labels).not.toContain('catalog.url_edit');
  });

  it('completes roles, events, and tags', async () => {
    expect(
      (await complete(`${header}[publication]\nretain_on_edit_roles = ["|"]`))
        .labels,
    ).toEqual(['catalog_manager']);
    expect(
      (
        await complete(
          `${header}[[rules]]\n[[rules.triggers]]\ntype = "event"\nevent_type = "|"`,
        )
      ).labels,
    ).toContain('attribute_value.changed.v1');
    expect(
      (await complete(`${header}[[attributes]]\ncode = "a"\ntags = ["hid|"]`))
        .labels,
    ).toContain('hidden:form');
  });

  it('completes table headers from the current section', async () => {
    const top = await complete(`${header}[[|`);
    expect(top.labels).toEqual(
      expect.arrayContaining([
        'attributes',
        'includes',
        'rules',
        'connector_jobs',
      ]),
    );
    expect(top.suggestions[0].insertText.endsWith(']]')).toBe(true);
    const nested = await complete(
      `${header}[views.detail]\ntype = "tabs"\n[[|]]`,
    );
    expect(nested.labels).toContain('views.detail.tabs');
    expect(
      nested.suggestions.find(
        (suggestion) => suggestion.label === 'views.detail.tabs',
      )?.insertText,
    ).toBe('views.detail.tabs');
  });

  it('uses the reusable attribute contract', async () => {
    const { labels } = await complete(
      'code = "brand"\nname = "Brand"\nvalue_type = "relationship"\n|',
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
      'code = "brand"\nname = "Brand"\nvalue_type = "|"',
      definitionKinds.reusableAttribute,
    );
    expect(values.labels).not.toContain('json');
  });
});
