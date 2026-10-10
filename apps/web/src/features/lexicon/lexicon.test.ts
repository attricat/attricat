import { afterEach, beforeAll, describe, expect, it } from 'vitest';
import i18n from '../../i18n';
import { LEXICON_NAMESPACE } from './constants';
import { lexiconCountNoun, lexiconText } from './lexicon';
import { lexiconResources } from './resources';
import type { LexiconEntry } from './schemas';

const entry = (
  key: string,
  language: string,
  text: string,
  pluralCategory = 'other',
  context: string | null = null,
): LexiconEntry => ({
  key,
  context,
  language,
  plural_category: pluralCategory,
  text,
});

const load = (language: string, entries: LexiconEntry[]) =>
  i18n.addResourceBundle(
    language,
    LEXICON_NAMESPACE,
    lexiconResources(entries),
    false,
    true,
  );

describe('lexiconResources', () => {
  it('uses suffixed plural keys and a singular bare key', () => {
    expect(
      lexiconResources([
        entry('Product', 'pl', 'produkt', 'one'),
        entry('Product', 'pl', 'produkty', 'few'),
        entry('Price', 'pl', 'Cena'),
        entry('Order', 'pl', 'Kolejność', 'other', 'sorting'),
      ]),
    ).toEqual({
      Product: 'produkt',
      Product_one: 'produkt',
      Product_few: 'produkty',
      Price: 'Cena',
      Order_sorting: 'Kolejność',
    });
  });
});

describe('lexiconText', () => {
  beforeAll(() => {
    load('en', [
      entry('Product', 'en', 'Product', 'one'),
      entry('Product', 'en', 'Products', 'other'),
      entry('Colour', 'en', 'Color'),
    ]);
    load('pl', [
      entry('Product', 'pl', 'produkt', 'one'),
      entry('Product', 'pl', 'produkty', 'few'),
      entry('Product', 'pl', 'produktów', 'many'),
      entry('Product', 'pl', 'produktu', 'other'),
      entry('Price', 'pl', 'Cena {{amount}}'),
      entry('Order', 'pl', 'Zamówienie', 'other', 'purchase'),
      entry('Status.code: x', 'pl', 'Kod'),
    ]);
  });

  afterEach(async () => {
    await i18n.changeLanguage('en');
  });

  it('resolves references in the UI language, then English, then the key', async () => {
    await i18n.changeLanguage('pl');
    expect(lexiconText('{{Price}}')).toBe('Cena {{amount}}');
    expect(lexiconText('{{Colour}}')).toBe('Color');
    expect(lexiconText('{{Untranslated}}')).toBe('Untranslated');
    expect(lexiconText('SKU {{Status.code: x}} \\{{x}}')).toBe('SKU Kod {{x}}');
  });

  it('does not fall back from a context to the plain key', async () => {
    await i18n.changeLanguage('pl');
    expect(lexiconText('{{Order|purchase}}')).toBe('Zamówienie');
    expect(lexiconText('{{Order|sorting}}')).toBe('Order');
  });

  it('keeps literal and malformed text as authored', () => {
    expect(lexiconText('Brand™')).toBe('Brand™');
    expect(lexiconText('{{Product')).toBe('{{Product');
  });

  it('uses the singular form without a count and plural forms with one', async () => {
    expect(lexiconText('{{Product}}')).toBe('Product');
    expect(lexiconCountNoun('{{Product}}', 3)).toBe('Products');
    await i18n.changeLanguage('pl');
    expect(lexiconText('{{Product}}')).toBe('produkt');
    expect(lexiconCountNoun('{{Product}}', 1)).toBe('produkt');
    expect(lexiconCountNoun('{{Product}}', 3)).toBe('produkty');
    expect(lexiconCountNoun('{{Product}}', 5)).toBe('produktów');
    expect(lexiconCountNoun('{{Product}}', 1.5)).toBe('produktu');
  });

  it('has no count noun without plural forms or for compound text', () => {
    expect(lexiconCountNoun('{{Colour}}', 3)).toBeUndefined();
    expect(lexiconCountNoun('Product', 3)).toBeUndefined();
    expect(lexiconCountNoun('My {{Product}}', 3)).toBeUndefined();
  });
});
