import { describe, expect, it } from 'vitest';
import {
  canonicalLanguage,
  languageLabel,
  pluralCategories,
  pluralExamples,
} from './languages';

describe('lexicon languages', () => {
  it('canonicalizes tags and rejects malformed ones', () => {
    expect(canonicalLanguage(' pt-br ')).toBe('pt-BR');
    expect(canonicalLanguage('not a tag')).toBeUndefined();
    expect(languageLabel('pl', 'en')).toBe('Polish (pl)');
  });

  it('lists CLDR plural categories with example counts', () => {
    expect(pluralCategories('en')).toEqual(['one', 'other']);
    expect(pluralCategories('pl')).toEqual(['one', 'few', 'many', 'other']);
    expect(pluralExamples('pl', 'few')).toEqual([2, 3, 4]);
    expect(pluralExamples('pl', 'many')).toEqual([0, 5, 6]);
    expect(pluralExamples('pl', 'other')).toEqual([1.5]);
  });
});
