import { describe, expect, it } from 'vitest';
import { scanToml } from './tomlOutline';

const cursorMarker = '|';

/** Scans `source` with the cursor at the `|` marker. */
const scanAtCursor = (source: string) => {
  const cursor = source.indexOf(cursorMarker);
  const text = source.replace(cursorMarker, '');
  return { outline: scanToml(text, cursor), text };
};

const definition = `format_version = 1
code = "product"
kind = "record"

[views.detail]
type = "stack"
children = [
  { type = "field", field = "name" },
  { type = "heading", text = "Stock" },
]

[[attributes]]
code = "name"
value_type = "string"

[[attributes]]
code = "category"
value_type = "relationship"
renderer.id = "attricat.table_image"
`;

describe('scanToml', () => {
  it('resolves array-of-table indices and inline collections', () => {
    const { valueAt, keysIn } = scanToml(definition);
    expect(valueAt(['code'])).toBe('product');
    expect(valueAt(['format_version'])).toBe(1);
    expect(valueAt(['attributes', 1, 'value_type'])).toBe('relationship');
    expect(valueAt(['views', 'detail', 'children', 1, 'text'])).toBe('Stock');
    expect(keysIn(['attributes', 1])).toEqual(
      new Set(['code', 'value_type', 'renderer']),
    );
    expect(keysIn([])).toEqual(
      new Set(['format_version', 'code', 'kind', 'views', 'attributes']),
    );
  });

  it('maps paths to their nearest source range', () => {
    const { rangeOf } = scanToml(definition);
    const range = rangeOf(['attributes', 1, 'value_type'])!;
    expect(definition.slice(range.start, range.end)).toBe('value_type');
    const header = rangeOf(['attributes', 1, 'missing'])!;
    expect(definition.slice(header.start, header.end)).toBe('[[attributes]]');
  });

  it('reports a key position on a blank line in the current table', () => {
    const { outline } = scanAtCursor('[[attributes]]\ncode = "a"\n|\n');
    expect(outline.cursor).toMatchObject({
      inline: false,
      kind: 'key',
      table: ['attributes', 0],
    });
  });

  it('reports a partially typed key and its replacement range', () => {
    const { outline, text } = scanAtCursor('[[attributes]]\nval|\n');
    expect(outline.cursor).toMatchObject({ kind: 'key' });
    const { range } = outline.cursor!;
    expect(text.slice(range.start, range.end)).toBe('val');
  });

  it('reports dotted keys against their parent table', () => {
    const { outline } = scanAtCursor('[[attributes]]\nrenderer.|\n');
    expect(outline.cursor).toMatchObject({
      kind: 'key',
      table: ['attributes', 0, 'renderer'],
    });
  });

  it('reports quoted and empty values', () => {
    expect(scanAtCursor('kind = "en|"').outline.cursor).toMatchObject({
      kind: 'value',
      path: ['kind'],
      quoted: true,
    });
    expect(scanAtCursor('kind = |').outline.cursor).toMatchObject({
      kind: 'value',
      path: ['kind'],
      quoted: false,
    });
  });

  it('reports array elements and inline-table keys', () => {
    expect(scanAtCursor('fields = ["name", |]').outline.cursor).toMatchObject({
      kind: 'value',
      path: ['fields', 1],
    });
    expect(
      scanAtCursor('children = [{ type = "field", | }]').outline.cursor,
    ).toMatchObject({ inline: true, kind: 'key', table: ['children', 0] });
    expect(
      scanAtCursor('component = { id = "attricat.|" }').outline.cursor,
    ).toMatchObject({ kind: 'value', path: ['component', 'id'] });
  });

  it('reports header positions with the preceding table', () => {
    expect(
      scanAtCursor('[views.detail]\ntype = "stack"\n[[vi|').outline.cursor,
    ).toMatchObject({
      array: true,
      kind: 'header',
      table: ['views', 'detail'],
    });
  });

  it('recovers when an array is left open while typing', () => {
    const { outline } = scanAtCursor(
      'fields = ["na|\n\n[[attributes]]\ncode = "name"\n',
    );
    expect(outline.cursor).toMatchObject({ kind: 'value', quoted: true });
    expect(outline.valueAt(['attributes', 0, 'code'])).toBe('name');
    const open = scanAtCursor('fields = [|\n[[attributes]]\ncode = "name"\n');
    expect(open.outline.cursor).toMatchObject({ path: ['fields', 0] });
    expect(open.outline.valueAt(['attributes', 0, 'code'])).toBe('name');
  });
});
