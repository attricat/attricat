import { describe, expect, it } from 'vitest';
import {
  applySuggestions,
  defaultFromDependents,
  maySuggest,
  suggestionSources,
  withoutEdited,
  type FormSuggestions,
} from './formSuggestions';

const noEdits: ReadonlySet<string> = new Set();
const defaulted: FormSuggestions = {
  price: { value: '10', source: suggestionSources.defaultFrom, recordId: 'a' },
};

describe('maySuggest', () => {
  it('fills empty fields and replaces only unedited suggestions', () => {
    const fields = { price: '10', note: 'typed' };
    expect(
      maySuggest(
        'price',
        suggestionSources.defaultFrom,
        fields,
        defaulted,
        noEdits,
      ),
    ).toBe(true);
    expect(
      maySuggest('empty', suggestionSources.defaultFrom, fields, {}, noEdits),
    ).toBe(true);
    // A value the user typed, without a suggestion, stays.
    expect(
      maySuggest('note', suggestionSources.assist, fields, {}, noEdits),
    ).toBe(false);
    // An edited field stays even when it is empty again.
    expect(
      maySuggest(
        'empty',
        suggestionSources.defaultFrom,
        fields,
        {},
        new Set(['empty']),
      ),
    ).toBe(false);
  });

  it('lets an assist replace a default, never the other way round', () => {
    const fields = { price: '10' };
    const assisted: FormSuggestions = {
      price: { value: '10', source: suggestionSources.assist },
    };
    expect(
      maySuggest('price', suggestionSources.assist, fields, defaulted, noEdits),
    ).toBe(true);
    expect(
      maySuggest(
        'price',
        suggestionSources.defaultFrom,
        fields,
        assisted,
        noEdits,
      ),
    ).toBe(false);
  });
});

describe('applySuggestions', () => {
  it('fills, refills and empties the suggestions of one source', () => {
    const filled = applySuggestions({
      edited: noEdits,
      fields: { price: '', vat: '' },
      proposals: [{ attributeCode: 'price', value: 12.5, recordId: 'a' }],
      scope: ['price', 'vat'],
      source: suggestionSources.defaultFrom,
      suggestions: {},
    });
    expect(filled?.fields).toEqual({ price: '12.5', vat: '' });
    expect(filled?.suggestions.price).toEqual({
      value: '12.5',
      source: suggestionSources.defaultFrom,
      recordId: 'a',
    });

    // Another target without a price empties the earlier default.
    const emptied = applySuggestions({
      edited: noEdits,
      fields: filled!.fields,
      proposals: [],
      scope: ['price', 'vat'],
      source: suggestionSources.defaultFrom,
      suggestions: filled!.suggestions,
    });
    expect(emptied?.fields).toEqual({ price: '', vat: '' });
    expect(emptied?.suggestions).toEqual({});
  });

  it('keeps edited fields and fields outside its scope', () => {
    expect(
      applySuggestions({
        edited: new Set(['price']),
        fields: { price: '9', other: '' },
        proposals: [
          { attributeCode: 'price', value: 12 },
          { attributeCode: 'other', value: 1 },
        ],
        scope: ['price'],
        source: suggestionSources.defaultFrom,
        suggestions: {},
      }),
    ).toBeUndefined();
  });

  it('reports no change for the same suggestion', () => {
    expect(
      applySuggestions({
        edited: noEdits,
        fields: { price: '10' },
        proposals: [{ attributeCode: 'price', value: 10, recordId: 'a' }],
        scope: ['price'],
        source: suggestionSources.defaultFrom,
        suggestions: defaulted,
      }),
    ).toBeUndefined();
  });
});

it('forgets the suggestions of edited fields', () => {
  expect(withoutEdited(defaulted, ['price'])).toEqual({});
  expect(withoutEdited(defaulted, ['note'])).toBe(defaulted);
});

it('finds the attributes defaulting from a relationship', () => {
  expect(
    defaultFromDependents(
      [
        { code: 'price', value_type: 'number', default_from: 'item.price' },
        { code: 'vat', value_type: 'number', default_from: 'tax.rate' },
        { code: 'item', value_type: 'relationship' },
      ],
      'item',
    ).map((attribute) => attribute.code),
  ).toEqual(['price']);
});
