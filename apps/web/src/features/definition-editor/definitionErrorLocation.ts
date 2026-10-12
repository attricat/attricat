import { scanToml, type TextRange, type TomlPath } from './tomlOutline';

const quotedNames = /'([^']+)'/g;
/** Leading definition keys that server messages name without quotes. */
const keyPrefix = /^([a-z_]+(?:\.[a-z_]+)*)\b/;

const within = (path: TomlPath, scope: string | undefined) =>
  scope === undefined || path.includes(scope);

const lastMatch = <T>(items: readonly T[], matches: (item: T) => boolean) => {
  for (let index = items.length - 1; index >= 0; index--)
    if (matches(items[index])) return items[index];
  return undefined;
};

/**
 * Finds where a server-side definition error most likely points. Messages name
 * what they reject in single quotes (`view 'detail' references unknown
 * attribute 'sku'`), so the last quoted name is matched against the
 * definition's values and keys, preferring places inside an earlier quoted
 * name. Messages that start with a key (`code is immutable`) point at it.
 */
export const locateDefinitionError = (
  text: string,
  message: string,
): TextRange | undefined => {
  const outline = scanToml(text);
  const names = [...message.matchAll(quotedNames)].map((match) => match[1]);
  for (let index = names.length - 1; index >= 0; index--) {
    const name = names[index];
    const scopes = [...names.slice(0, index).reverse(), undefined];
    for (const scope of scopes) {
      const value = lastMatch(
        outline.entries,
        (entry) => entry.value === name && within(entry.path, scope),
      );
      if (value) return value.range;
      const key = lastMatch(
        [...outline.entries, ...outline.headers],
        ({ path }) => path.at(-1) === name && within(path, scope),
      );
      if (key) return key.range;
    }
  }
  const key = message.match(keyPrefix)?.[1];
  return key ? outline.rangeOf(key.split('.')) : undefined;
};
