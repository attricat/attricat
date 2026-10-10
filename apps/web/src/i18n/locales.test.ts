import { describe, expect, it } from 'vitest';
import en from './locales/en.json';
import pl from './locales/pl.json';

const leafKeys = (value: unknown, prefix = ''): string[] => {
  if (!value || typeof value !== 'object' || Array.isArray(value))
    return [prefix];
  return Object.entries(value).flatMap(([key, child]) =>
    leafKeys(child, prefix ? `${prefix}.${key}` : key),
  );
};

const pluralBase = (key: string) => key.replace(/_(one|few|many|other)$/, '');

describe('translations', () => {
  it('keeps English and Polish translation keys aligned', () => {
    const english = new Set(leafKeys(en));
    const polish = new Set(leafKeys(pl));

    expect([...english].filter((key) => !polish.has(key))).toEqual([]);
    const englishBases = new Set([...english].map(pluralBase));
    expect(
      [...polish].filter((key) => !englishBases.has(pluralBase(key))),
    ).toEqual([]);
  });
});
