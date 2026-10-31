import {
  isScalar,
  parseDocument,
  Scalar,
  visit,
  type Node,
  type ParsedNode,
  type ScalarTag,
} from 'yaml';

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

const yamlTag = (name: string) => `tag:yaml.org,2002:${name}`;

/** Digits with single underscores between them, as in `1_000`. */
const digitGroups = (digit: string) => `${digit}+(?:_${digit}+)*`;

/** An integer in a radix, by the prefix that introduces it. */
const radixes = [
  { digits: digitGroups('[0-9a-f]'), prefix: '0x', radix: 16 },
  { digits: digitGroups('[0-7]'), prefix: '0o', radix: 8 },
  { digits: digitGroups('[01]'), prefix: '0b', radix: 2 },
] as const;
/** The prefix of an explicit radix: `0x`, `0o` or `0b`, in either case. */
const radixPrefixLength = 2;

/**
 * Types plain scalars as the API's parser (serde-saphyr with strict booleans)
 * does, rather than as the YAML 1.2 core schema does: `true` and `false` in
 * any letter case are the only booleans; `null` in any case, `~` and nothing
 * are null; integers may be signed, separate digits with `_` and use `0x`,
 * `0o` or `0b` in either case; floats that are not finite are rejected by
 * `parseStrictYaml`, as are decimal integers with a leading zero.
 */
const definitionScalarTags: ScalarTag[] = [
  {
    default: true,
    resolve: () => null,
    tag: yamlTag('null'),
    test: /^(?:~|null)?$/i,
  },
  {
    default: true,
    resolve: (value) => value.toLowerCase() === 'true',
    tag: yamlTag('bool'),
    test: /^(?:true|false)$/i,
  },
  ...radixes.map(({ digits, prefix, radix }): ScalarTag => ({
    default: true,
    resolve: (value) => {
      const unsigned = value.replace(/^[-+]/, '');
      const magnitude = parseInt(
        unsigned.slice(radixPrefixLength).replaceAll('_', ''),
        radix,
      );
      return value.startsWith('-') ? -magnitude : magnitude;
    },
    tag: yamlTag('int'),
    test: new RegExp(`^[-+]?${prefix}${digits}$`, 'i'),
  })),
  {
    default: true,
    resolve: (value) => Number(value.replaceAll('_', '')),
    tag: yamlTag('int'),
    // A leading zero, as in `012`, is read so that the API's rule rejects it.
    test: /^[-+]?(?:[0-9]+|[1-9](?:_?[0-9])*)$/,
  },
  {
    default: true,
    resolve: (value) => Number(value),
    tag: yamlTag('float'),
    test: /^[-+]?(?:[0-9]+(?:\.[0-9]*)?|\.[0-9]+)(?:[eE][-+]?[0-9]+)?$/,
  },
  {
    default: true,
    resolve: (value) =>
      value.toLowerCase().endsWith('nan')
        ? Number.NaN
        : value.startsWith('-')
          ? Number.NEGATIVE_INFINITY
          : Number.POSITIVE_INFINITY,
    tag: yamlTag('float'),
    test: /^[-+]?\.(?:inf|nan)$/i,
  },
];

/**
 * Parser options that type scalars as the API does; the failsafe schema
 * supplies mappings, sequences and strings. Keys are read as written, so
 * `012:` names `012`, as it does for the API.
 */
export const definitionYamlOptions = {
  customTags: definitionScalarTags,
  schema: 'failsafe',
  stringKeys: true,
} as const;

/** Rules the API checks beyond YAML syntax, each reported by its code. */
export const yamlPolicyErrors = {
  integerLeadingZero: 'integerLeadingZero',
  integerOutOfRange: 'integerOutOfRange',
  nonFiniteFloat: 'nonFiniteFloat',
  unsupportedTag: 'unsupportedTag',
} as const;
export type YamlPolicyError =
  (typeof yamlPolicyErrors)[keyof typeof yamlPolicyErrors];

export type YamlSyntaxError = {
  column: number;
  line: number;
  message: string;
  /** The API's rule the text breaks, and the value or tag that breaks it. */
  policy?: { code: YamlPolicyError; value: string };
};

const coreTagPrefix = 'tag:yaml.org,2002:';
/** The non-specific tag `!`, which keeps a scalar a string. */
const nonSpecificTag = '!';
/** Core-schema tags a definition may name, as the API accepts them. */
const supportedTags = new Set(
  ['str', 'int', 'float', 'bool', 'null', 'seq', 'map'].map(yamlTag),
);
const decimalInteger = /^[-+]?[0-9]+$/;
const int64Min = -(2n ** 63n);
const uint64Max = 2n ** 64n - 1n;

/**
 * The first value breaking a rule the API checks before reading values:
 * only core-schema and `!` tags, decimal integers without a leading zero
 * and within 64 bits, and floats that are finite.
 */
const policyError = (document: ReturnType<typeof parseDocument>) => {
  let found: { node: Node; policy: YamlSyntaxError['policy'] & {} } | undefined;
  const check = (node: Node, value: unknown) => {
    const { tag } = node;
    if (tag && tag !== nonSpecificTag && !supportedTags.has(tag))
      found = {
        node,
        policy: {
          code: yamlPolicyErrors.unsupportedTag,
          value: tag.startsWith(coreTagPrefix)
            ? `!!${tag.slice(coreTagPrefix.length)}`
            : tag,
        },
      };
    else if (!isScalar(node)) return;
    else if (
      decimalInteger.test(node.source ?? '') &&
      (tag === yamlTag('int') || (!tag && node.type === Scalar.PLAIN))
    ) {
      const source = node.source!;
      const digits = source.replace(/^[-+]/, '');
      const integer = BigInt(source);
      if (digits.length > 1 && digits.startsWith('0'))
        found = {
          node,
          policy: { code: yamlPolicyErrors.integerLeadingZero, value: source },
        };
      else if (integer < int64Min || integer > uint64Max)
        found = {
          node,
          policy: { code: yamlPolicyErrors.integerOutOfRange, value: source },
        };
    } else if (typeof value === 'number' && !Number.isFinite(value))
      found = {
        node,
        policy: {
          code: yamlPolicyErrors.nonFiniteFloat,
          value: node.source ?? String(value),
        },
      };
    return found ? visit.BREAK : undefined;
  };
  visit(document, {
    Collection: (_, node) => check(node, undefined),
    // Keys are only compared, so only their tags matter.
    Scalar: (key, node) =>
      key === 'key' && !node.tag ? undefined : check(node, node.value),
  });
  return found;
};

/** English messages, as the API words them. */
const policyMessages: Record<YamlPolicyError, (value: string) => string> = {
  integerLeadingZero: (value) =>
    `integer \`${value}\` has a leading zero; remove it, or quote the value to keep it as text`,
  integerOutOfRange: (value) =>
    `integer \`${value}\` is outside the 64-bit range; quote the value to keep it as text`,
  nonFiniteFloat: (value) =>
    `non-finite float \`${value}\` rejected by the configured policy`,
  unsupportedTag: (value) => `unsupported tag \`${value}\``,
};

/** Keys are duplicates when written alike, such as `1` and `"1"`. */
const sameKey = (a: ParsedNode, b: ParsedNode) =>
  a === b || (isScalar(a) && isScalar(b) && a.source === b.source);

/** The 1-based line and column of `offset`. */
const lineColumn = (text: string, offset: number) => {
  const lineStart = text.lastIndexOf('\n', offset - 1) + 1;
  return {
    column: offset - lineStart + 1,
    line: text.slice(0, lineStart).split('\n').length,
  };
};

/**
 * Parses YAML the way the API does: duplicate keys are errors, scalars are
 * typed by `definitionYamlOptions`, `<<` is an ordinary key, and the API's
 * rules for tags, integers and floats in `yamlPolicyErrors` apply.
 */
export const parseStrictYaml = (
  text: string,
): { data: unknown } | { error: YamlSyntaxError } => {
  const document = parseDocument(text, {
    ...definitionYamlOptions,
    merge: false,
    uniqueKeys: sameKey,
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
  const broken = policyError(document);
  if (broken) {
    const start = broken.node.range?.[0] ?? 0;
    // The API points at a tag, which comes before the node it names.
    const tagStart =
      broken.policy.code === yamlPolicyErrors.unsupportedTag
        ? text.lastIndexOf(broken.policy.value, start)
        : -1;
    return {
      error: {
        ...lineColumn(text, tagStart < 0 ? start : tagStart),
        message: policyMessages[broken.policy.code](broken.policy.value),
        policy: broken.policy,
      },
    };
  }
  return { data: document.toJS() ?? {} };
};
