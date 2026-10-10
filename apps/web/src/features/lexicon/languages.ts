import type { TFunction } from 'i18next';
import {
  PLURAL_CATEGORIES,
  PLURAL_EXAMPLE_COUNT,
  PLURAL_EXAMPLE_SCAN_LIMIT,
  PLURAL_FRACTION_EXAMPLE,
} from './constants';

/** The BCP 47 canonical form of `tag`, or `undefined` when it is malformed. */
export const canonicalLanguage = (tag: string): string | undefined => {
  try {
    return Intl.getCanonicalLocales(tag.trim())[0];
  } catch {
    return undefined;
  }
};

/** "Polish (pl)" in the UI language, falling back to the bare tag. */
export const languageLabel = (tag: string, uiLanguage: string) => {
  try {
    const name = new Intl.DisplayNames([uiLanguage], { type: 'language' }).of(
      tag,
    );
    return name && name !== tag ? `${name} (${tag})` : tag;
  } catch {
    return tag;
  }
};

/** The CLDR plural categories `language` uses, in CLDR order. */
export const pluralCategories = (language: string): string[] => {
  const used = new Set<string>(
    new Intl.PluralRules(language).resolvedOptions().pluralCategories,
  );
  return PLURAL_CATEGORIES.filter((category) => used.has(category));
};

/** The translated name of a CLDR plural category; unknown categories show as-is. */
export const pluralCategoryLabel = (t: TFunction, category: string) =>
  t(`lexicon.pluralCategories.${category}`, { defaultValue: category });

/** Sample counts that select `category` in `language`, such as 2, 3, 4 for Polish `few`. */
export const pluralExamples = (language: string, category: string) => {
  const rules = new Intl.PluralRules(language);
  const examples: number[] = [];
  for (
    let count = 0;
    count <= PLURAL_EXAMPLE_SCAN_LIMIT &&
    examples.length < PLURAL_EXAMPLE_COUNT;
    count += 1
  )
    if (rules.select(count) === category) examples.push(count);
  if (!examples.length && rules.select(PLURAL_FRACTION_EXAMPLE) === category)
    examples.push(PLURAL_FRACTION_EXAMPLE);
  return examples;
};
