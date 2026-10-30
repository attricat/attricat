import { parseDocument } from 'yaml';

/**
 * Words that YAML reads as something other than a plain string in at least
 * one common schema, so strings spelled like them are quoted.
 */
const reservedWords = new Set([
  '~',
  'false',
  'n',
  'no',
  'null',
  'off',
  'on',
  'true',
  'y',
  'yes',
]);

/** Plain scalars start with a letter or one of a few safe characters. */
const plainStart = /^[A-Za-z_/$(]/;
const numberLike = /^[-+.]?\d|^0[xo]|^[-+]?\.(inf|nan)$/i;
const flowIndicators = /[,[\]{}]/;

/**
 * Whether a string can be written unquoted. Flow collections (`[…]`, `{…}`)
 * also reserve commas and brackets.
 */
const isPlain = (value: string, flow: boolean) =>
  plainStart.test(value) &&
  value === value.trim() &&
  !reservedWords.has(value.toLowerCase()) &&
  !numberLike.test(value) &&
  !/[\n\t]/.test(value) &&
  !value.includes(': ') &&
  !value.includes(' #') &&
  !value.endsWith(':') &&
  !(flow && flowIndicators.test(value));

/** A string scalar, single-quoted only when it has to be. */
export const yamlString = (value: string, flow = false) =>
  isPlain(value, flow)
    ? value
    : /[\n\t\\]/.test(value)
      ? JSON.stringify(value)
      : `'${value.replaceAll("'", "''")}'`;

const bareKey = /^[A-Za-z0-9_-]+$/;

/** A mapping key, quoted when it is not a plain identifier. */
export const yamlKey = (key: string) =>
  bareKey.test(key) && !numberLike.test(key) ? key : yamlString(key, true);

/** A flow mapping's text, spaced inside its braces as Prettier writes it. */
export const yamlFlowMapping = (entries: string[]) =>
  entries.length ? `{ ${entries.join(', ')} }` : '{}';

/** A value in flow style, as written after `key: ` on one line. */
export const yamlValue = (value: unknown): string => {
  if (Array.isArray(value)) return `[${value.map(yamlValue).join(', ')}]`;
  if (value !== null && typeof value === 'object')
    return yamlFlowMapping(
      Object.entries(value).map(
        ([key, item]) => `${yamlKey(key)}: ${yamlValue(item)}`,
      ),
    );
  if (typeof value === 'string') return yamlString(value, true);
  return value === null || value === undefined ? 'null' : String(value);
};

export type YamlSyntaxError = { column: number; line: number; message: string };

/**
 * Parses YAML the way the API does: duplicate keys are errors, only `true`
 * and `false` are booleans, and `<<` is an ordinary key.
 */
export const parseStrictYaml = (
  text: string,
): { data: unknown } | { error: YamlSyntaxError } => {
  const document = parseDocument(text, {
    merge: false,
    schema: 'core',
    uniqueKeys: true,
  });
  const [error] = document.errors;
  if (error) {
    const position = error.linePos?.[0] ?? { col: 1, line: 1 };
    return {
      error: {
        column: position.col,
        line: position.line,
        message: error.message
          .split('\n')[0]!
          .replace(/ at line \d+, column \d+:?$/, ''),
      },
    };
  }
  return { data: document.toJS() ?? {} };
};
