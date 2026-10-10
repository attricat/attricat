import { describe, expect, it } from 'vitest';
import type { Attribute } from '../records/api';
import {
  firstQueryError,
  highlightQuery,
  queryCursorContext,
  type QuerySchema,
} from './queryLanguage';
import {
  applyQuerySuggestion,
  querySuggestions,
  queryTargetCodes,
} from './querySuggestions';

const scalar = (code: string, value_type: Attribute['value_type'] = 'string') =>
  ({ code, value_type }) satisfies Attribute;
const relationship = (code: string, target: string) =>
  ({
    code,
    value_type: 'relationship',
    target_blueprint_code: target,
  }) satisfies Attribute;

const schema: QuerySchema = {
  blueprint: { code: 'product', name: 'Product' },
  attributes: [
    scalar('sku'),
    scalar('title'),
    scalar('active', 'boolean'),
    scalar('photo', 'file'),
    relationship('colors', 'color'),
    relationship('category', 'category'),
  ],
  targets: new Map([
    ['color', [scalar('name'), scalar('swatch', 'json')]],
    ['category', [scalar('name'), relationship('parent', 'category')]],
  ]),
};
const names = new Map([
  ['color', 'Color'],
  ['category', 'Category'],
]);

const kinds = (query: string) =>
  highlightQuery(query, schema).map(({ kind, text, error }) => [
    kind,
    text,
    error?.code,
  ]);

describe('highlightQuery', () => {
  it('covers every character of the query', () => {
    const query = '  linen  colors.name:red*  *:blue ';
    expect(
      highlightQuery(query, schema)
        .map((segment) => segment.text)
        .join(''),
    ).toBe(query);
  });

  it('classifies selectors, punctuation, values, and wildcards', () => {
    expect(kinds('category.parent.name:sum*')).toEqual([
      ['field', 'category', undefined],
      ['punctuation', '.', undefined],
      ['field', 'parent', undefined],
      ['punctuation', '.', undefined],
      ['field', 'name', undefined],
      ['punctuation', ':', undefined],
      ['value', 'sum', undefined],
      ['wildcard', '*', undefined],
    ]);
    expect(kinds('*:red')[0]).toEqual(['global', '*', undefined]);
  });

  it('accepts the blueprint alias and case-insensitive codes', () => {
    expect(
      firstQueryError(
        highlightQuery('Product:x SKU:1 product.sku:2', schema),
        '',
      ),
    ).toBeUndefined();
  });

  it('flags unknown fields and relationships like the server', () => {
    expect(kinds('color:red')[0]).toEqual(['field', 'color', 'unknownField']);
    expect(kinds('sku.name:x')[0]).toEqual([
      'field',
      'sku',
      'unknownRelationship',
    ]);
    expect(kinds('colors.swatch:x')[2]).toEqual([
      'field',
      'swatch',
      'notSearchableLeaf',
    ]);
    expect(
      firstQueryError(
        highlightQuery('category.parent.parent.parent.name:x', schema),
        '',
      ),
    ).toEqual({ code: 'tooManyHops' });
  });

  it('flags misplaced wildcards and incomplete terms', () => {
    expect(kinds('a*b')[0][2]).toBe('misplacedWildcard');
    expect(kinds('sku:')[1][2]).toBe('malformedTerm');
    expect(kinds('sku:a:b')[2][2]).toBe('malformedTerm');
  });

  it('does not report the incomplete term being typed', () => {
    const query = 'linen sku:';
    const segments = highlightQuery(query, schema);
    expect(firstQueryError(segments, query, query.length)).toBeUndefined();
    expect(firstQueryError(segments, query)).toEqual({
      code: 'malformedTerm',
      term: 'sku:',
    });
  });

  it('accepts ID lists on the blueprint and through relationships', () => {
    const id = '0190a6f2-7c1e-7b3a-9c4d-2e5f6a7b8c9d';
    const other = '0190a6f2-7c1e-7b3a-9c4d-2e5f6a7b8c9e';
    expect(
      firstQueryError(
        highlightQuery(
          `@id:${id},${other} Product.@ID:${id} category.parent.@id:${id}`,
          schema,
        ),
        '',
      ),
    ).toBeUndefined();
    expect(kinds(`colors.@id:${id}`)).toEqual([
      ['field', 'colors', undefined],
      ['punctuation', '.', undefined],
      ['field', '@id', undefined],
      ['punctuation', ':', undefined],
      ['value', id, undefined],
    ]);
  });

  it('flags malformed ID lists and non-relationship ID paths', () => {
    const id = '0190a6f2-7c1e-7b3a-9c4d-2e5f6a7b8c9d';
    expect(kinds(`@id:${id},red`)[2][2]).toBe('invalidId');
    expect(kinds(`@id:${id},`)[2][2]).toBe('invalidId');
    expect(kinds(`@id:${id}*`)[2][2]).toBe('invalidId');
    expect(kinds(`sku.@id:${id}`)[0]).toEqual([
      'field',
      'sku',
      'unknownRelationship',
    ]);
    const tooMany = Array.from(
      { length: 101 },
      (_, index) =>
        `00000000-0000-0000-0000-${String(index).padStart(12, '0')}`,
    ).join(',');
    expect(
      firstQueryError(highlightQuery(`@id:${tooMany}`, schema), ''),
    ).toEqual({ code: 'tooManyIds', count: 100 });
  });

  it('treats relationship hops as valid while targets load', () => {
    const loading = { ...schema, targets: new Map() };
    expect(
      firstQueryError(highlightQuery('colors.name:red', loading), ''),
    ).toBeUndefined();
  });
});

describe('queryCursorContext', () => {
  it('finds the field or value under the cursor', () => {
    expect(queryCursorContext('linen col', 9)).toMatchObject({
      kind: 'field',
      path: [],
      partial: 'col',
      replaceStart: 6,
    });
    expect(queryCursorContext('colors.na', 9)).toMatchObject({
      kind: 'field',
      path: ['colors'],
      partial: 'na',
    });
    expect(queryCursorContext('sku:AB', 6)).toMatchObject({
      kind: 'value',
      selector: 'sku',
      partial: 'AB',
    });
  });
});

describe('querySuggestions', () => {
  const codes = (query: string, cursor = query.length) =>
    querySuggestions(schema, query, cursor, names).suggestions.map(
      (suggestion) => suggestion.code,
    );

  it('offers own fields, relationships, and global search', () => {
    expect(codes('')).toEqual([
      '*',
      'sku',
      'title',
      'active',
      'colors',
      'category',
      '@id',
    ]);
  });

  it('ranks prefix matches before substring matches', () => {
    expect(codes('t')).toEqual(['title', 'active', 'category']);
  });

  it('suggests related fields and all fields after a relationship', () => {
    expect(codes('colors.')).toEqual(['colors', 'colors.name', 'colors.@id']);
    expect(codes('category.pa')).toEqual(['category.parent']);
    expect(codes('category.parent.')).toEqual([
      'category.parent.name',
      'category.parent.parent',
      'category.parent.@id',
    ]);
    expect(codes('category.parent.parent.')).toEqual([
      'category.parent.parent.name',
      'category.parent.parent.@id',
    ]);
  });

  it('suggests boolean values', () => {
    expect(codes('active:')).toEqual(['true', 'false']);
    expect(codes('active:t')).toEqual(['true']);
    expect(codes('sku:')).toEqual([]);
  });

  it('reports loading while a relationship target is unknown', () => {
    const loading = { ...schema, targets: new Map() };
    expect(querySuggestions(loading, 'colors.', 7, names).loading).toBe(true);
  });
});

describe('applyQuerySuggestion', () => {
  const apply = (query: string, cursor: number, code: string) => {
    const result = querySuggestions(schema, query, cursor, names);
    const suggestion = result.suggestions.find((item) => item.code === code);
    if (!suggestion) throw new Error(`no suggestion ${code}`);
    return applyQuerySuggestion(query, result.context, suggestion);
  };

  it('completes a field and places the caret after the colon', () => {
    expect(apply('linen sk', 8, 'sku')).toEqual({
      query: 'linen sku:',
      cursor: 10,
    });
  });

  it('keeps an existing value when replacing a field', () => {
    expect(apply('ti:red', 1, 'title')).toEqual({
      query: 'title:red',
      cursor: 6,
    });
  });

  it('drills into relationships and turns `rel.` into `rel:`', () => {
    expect(apply('col', 3, 'colors')).toEqual({
      query: 'colors.',
      cursor: 7,
    });
    expect(apply('colors.', 7, 'colors')).toEqual({
      query: 'colors:',
      cursor: 7,
    });
    expect(apply('colors.n', 8, 'colors.name')).toEqual({
      query: 'colors.name:',
      cursor: 12,
    });
  });

  it('inserts a value followed by a space', () => {
    expect(apply('active:', 7, 'true')).toEqual({
      query: 'active:true ',
      cursor: 12,
    });
  });
});

describe('queryTargetCodes', () => {
  it('lists the targets every typed path walks through', () => {
    expect(
      queryTargetCodes(schema, 'linen category.parent.na colors:red'),
    ).toEqual(['category']);
    expect(queryTargetCodes(schema, 'colors.')).toEqual(['color']);
  });
});
