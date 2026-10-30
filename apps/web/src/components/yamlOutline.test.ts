import { describe, expect, it } from 'vitest';
import { scanYaml } from './yamlOutline';

const cursorMarker = '|';

/** Scans `source` with the cursor at the `|` marker. */
const scanAtCursor = (source: string) => {
  const cursor = source.indexOf(cursorMarker);
  const text = source.replace(cursorMarker, '');
  return { outline: scanYaml(text, cursor), text };
};

const definition = `format_version: 1
code: product
kind: record

views:
  detail:
    type: stack
    children:
      - {type: field, field: name}
      - {type: heading, text: Stock}

attributes:
  - code: name
    value_type: string

  - code: category
    value_type: relationship
    renderer:
      id: attricat.table_image
`;

describe('scanYaml', () => {
  it('resolves sequence indices and flow collections', () => {
    const { valueAt, keysIn } = scanYaml(definition);
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
    const { rangeOf } = scanYaml(definition);
    const range = rangeOf(['attributes', 1, 'value_type'])!;
    expect(definition.slice(range.start, range.end)).toBe('value_type');
    const parent = rangeOf(['attributes', 1, 'renderer', 'missing'])!;
    expect(definition.slice(parent.start, parent.end)).toBe('renderer');
  });

  it('reports a key position on a blank line in the current mapping', () => {
    expect(
      scanAtCursor('attributes:\n  - code: a\n    |\n').outline.cursor,
    ).toMatchObject({ inline: false, kind: 'key', mapping: ['attributes', 0] });
    expect(
      scanAtCursor('views:\n  detail:\n    type: stack\n  |\n').outline.cursor,
    ).toMatchObject({ kind: 'key', mapping: ['views'] });
    expect(
      scanAtCursor('code: x\nviews:\n  detail:\n    type: stack\n|\n').outline
        .cursor,
    ).toMatchObject({ kind: 'key', mapping: [] });
  });

  it('reports the first key of a new nested mapping', () => {
    expect(
      scanAtCursor('views:\n  detail:\n    |\n').outline.cursor,
    ).toMatchObject({ kind: 'key', mapping: ['views', 'detail'] });
  });

  it('reports a partially typed key and its replacement range', () => {
    const { outline, text } = scanAtCursor(
      'attributes:\n  - code: a\n    val|\n',
    );
    expect(outline.cursor).toMatchObject({
      kind: 'key',
      mapping: ['attributes', 0],
    });
    const { range } = outline.cursor!;
    expect(text.slice(range.start, range.end)).toBe('val');
  });

  it('reports keys that start a new sequence item', () => {
    expect(
      scanAtCursor('attributes:\n  - code: a\n  - |\n').outline.cursor,
    ).toMatchObject({ item: true, kind: 'key', mapping: ['attributes', 1] });
    expect(scanAtCursor('attributes:\n  - |\n').outline.cursor).toMatchObject({
      item: true,
      kind: 'key',
      mapping: ['attributes', 0],
    });
    expect(
      scanAtCursor('triggers:\n  - type: manual\n\n  - ty|\n').outline.cursor,
    ).toMatchObject({ item: true, kind: 'key', mapping: ['triggers', 1] });
  });

  it('reports quoted and empty values', () => {
    expect(scanAtCursor("kind: 'en|'").outline.cursor).toMatchObject({
      kind: 'value',
      path: ['kind'],
      quoted: true,
    });
    expect(scanAtCursor('kind: |').outline.cursor).toMatchObject({
      kind: 'value',
      path: ['kind'],
      quoted: false,
    });
    expect(
      scanAtCursor('attributes:\n  - code: a\n    value_type: str|\n').outline
        .cursor,
    ).toMatchObject({
      kind: 'value',
      path: ['attributes', 0, 'value_type'],
      quoted: false,
    });
  });

  it('reports the replacement range of a plain value', () => {
    const { outline, text } = scanAtCursor('kind: rec|ord  # comment\n');
    const { range } = outline.cursor!;
    expect(text.slice(range.start, range.end)).toBe('record');
  });

  it('reports flow sequence elements and flow mapping keys', () => {
    expect(scanAtCursor('fields: [name, |]').outline.cursor).toMatchObject({
      kind: 'value',
      path: ['fields', 1],
    });
    expect(
      scanAtCursor('children: [{type: field, |}]').outline.cursor,
    ).toMatchObject({ inline: true, kind: 'key', mapping: ['children', 0] });
    expect(
      scanAtCursor('component: {id: attricat.|}').outline.cursor,
    ).toMatchObject({ kind: 'value', path: ['component', 'id'] });
  });

  it('waits for the space after a colon or dash', () => {
    expect(scanAtCursor('kind:|').outline.cursor).toBeUndefined();
    expect(scanAtCursor('attributes:\n  -|').outline.cursor).toBeUndefined();
  });

  it('ignores comments', () => {
    expect(scanAtCursor('code: x  # note |\n').outline.cursor).toBeUndefined();
    expect(scanAtCursor('# note |\n').outline.cursor).toBeUndefined();
  });

  it('recovers when a flow sequence is left open while typing', () => {
    const { outline } = scanAtCursor(
      "fields: ['na|\n\nattributes:\n  - code: name\n",
    );
    expect(outline.cursor).toMatchObject({ kind: 'value', quoted: true });
    expect(outline.valueAt(['attributes', 0, 'code'])).toBe('name');
    const open = scanAtCursor('fields: [|\nattributes:\n  - code: name\n');
    expect(open.outline.cursor).toMatchObject({ path: ['fields', 0] });
  });
});
