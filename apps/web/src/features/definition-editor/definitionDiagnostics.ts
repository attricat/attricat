import Ajv2020, {
  type ErrorObject,
  type ValidateFunction,
} from 'ajv/dist/2020';
import i18n from '../../i18n';
import {
  codePattern,
  discriminatorKey,
  type DefinitionKind,
} from './constants';
import {
  choicesOf,
  defaultedVariants,
  definitionSchemas,
  primaryType,
  taggedVariants,
  unwrapSchema,
  variantTag,
  type SchemaNode,
} from './definitionSchema';
import {
  scanYaml,
  type TextRange,
  type YamlOutline,
  type YamlPath,
} from '../../components/yamlOutline';
import {
  parseStrictYaml,
  yamlKey,
  yamlValue,
} from '../../components/yamlSyntax';

/** A text change that resolves a diagnostic, offered as a quick fix. */
export type DefinitionFix = {
  edits: { range: TextRange; text: string }[];
  title: string;
};

export type DefinitionDiagnostic = {
  fix?: DefinitionFix;
  message: string;
  range: TextRange;
};

/** Keys this far apart or closer are offered as likely typos. */
const maxSuggestionDistance = 2;

/**
 * Adds Ajv `discriminator`s to tagged unions so a block with a known `type`
 * reports only its own variant's errors instead of every variant's.
 */
const withDiscriminators = (root: SchemaNode, node: unknown): unknown => {
  if (Array.isArray(node))
    return node.map((item) => withDiscriminators(root, item));
  if (node === null || typeof node !== 'object') return node;
  const copy = Object.fromEntries(
    Object.entries(node).map(([key, value]) => [
      key,
      withDiscriminators(root, value),
    ]),
  );
  if (taggedVariants(root, node as SchemaNode))
    return { ...copy, discriminator: { propertyName: discriminatorKey } };
  const defaulted = defaultedVariants(root, node as SchemaNode);
  if (!defaulted) return copy;
  // Without a `type`, only the default branch applies; with one, the tag
  // picks the branch, so errors come from that branch alone.
  const { oneOf, ...rest } = copy as { oneOf: unknown[] };
  return {
    ...rest,
    if: { required: [discriminatorKey] },
    then: {
      discriminator: { propertyName: discriminatorKey },
      oneOf: oneOf.filter((_, index) => index !== defaulted.fallbackIndex),
    },
    else: oneOf[defaulted.fallbackIndex],
  };
};

const validators = new Map<DefinitionKind, ValidateFunction>();

const validatorFor = (kind: DefinitionKind) => {
  let validator = validators.get(kind);
  if (!validator) {
    const root = definitionSchemas[kind];
    validator = new Ajv2020({
      allErrors: true,
      discriminator: true,
      strict: false,
      // schemars annotates integers with Rust formats such as `uint32`; their
      // bounds are also emitted as `minimum`.
      validateFormats: false,
      verbose: true,
    }).compile(withDiscriminators(root, root) as object);
    validators.set(kind, validator);
  }
  return validator;
};

const unescapePointer = (segment: string) =>
  segment.replaceAll('~1', '/').replaceAll('~0', '~');

/** Converts a JSON pointer into a YAML path, using the data to type indices. */
const pointerPath = (pointer: string, data: unknown): YamlPath => {
  const path: YamlPath = [];
  let current = data;
  for (const raw of pointer.split('/').slice(1)) {
    const segment = unescapePointer(raw);
    const key = Array.isArray(current) ? Number(segment) : segment;
    path.push(key);
    current = (current as Record<string | number, unknown> | undefined)?.[key];
  }
  return path;
};

const distance = (left: string, right: string) => {
  const previous = Array.from(
    { length: right.length + 1 },
    (_, index) => index,
  );
  for (let row = 1; row <= left.length; row++) {
    let diagonal = previous[0];
    previous[0] = row;
    for (let column = 1; column <= right.length; column++) {
      const above = previous[column];
      previous[column] = Math.min(
        previous[column] + 1,
        previous[column - 1] + 1,
        diagonal + (left[row - 1] === right[column - 1] ? 0 : 1),
      );
      diagonal = above;
    }
  }
  return previous[right.length];
};

const closestKey = (key: string, candidates: string[]) => {
  let best: string | undefined;
  let bestDistance = maxSuggestionDistance + 1;
  for (const candidate of candidates) {
    const candidateDistance = distance(key, candidate);
    if (candidateDistance < bestDistance) {
      best = candidate;
      bestDistance = candidateDistance;
    }
  }
  return best;
};

const listValues = (values: unknown[]) =>
  values.map((value) => JSON.stringify(value)).join(', ');

/** What would resolve an error, located in the text once it is known. */
type FixIntent =
  | { kind: 'renameKey'; to: string }
  | { kind: 'insertKey'; key: string; node: SchemaNode }
  | { kind: 'replaceValue'; choices: unknown[] };

type Located = { fix?: FixIntent; message: string; path: YamlPath };

/**
 * The description of the conditional rule (a schema with `if` and no
 * properties of its own) that an error's schema path passes through.
 */
const ruleDescription = (schema: unknown, schemaPath: string) => {
  let node = schema;
  let description: string | undefined;
  for (const segment of schemaPath.replace(/^#\//, '').split('/')) {
    node = (node as Record<string, unknown> | undefined)?.[
      unescapePointer(segment)
    ];
    if (
      node &&
      typeof node === 'object' &&
      'if' in node &&
      !('properties' in node) &&
      'description' in node &&
      typeof node.description === 'string'
    )
      description = node.description;
  }
  return description;
};

const describeError = (
  error: ErrorObject,
  path: YamlPath,
  schema: unknown,
): Located | undefined => {
  const t = i18n.t.bind(i18n);
  const params = error.params as Record<string, unknown>;
  const parentSchema = error.parentSchema as SchemaNode | undefined;
  switch (error.keyword) {
    // The failing branch reports its own errors.
    case 'if':
      return undefined;
    case 'not': {
      const forbidden = (parentSchema as { not?: SchemaNode } | undefined)?.not;
      const [key] = forbidden?.required ?? [];
      const rule = ruleDescription(schema, error.schemaPath);
      const message = key
        ? t('definitionEditor.diagnostics.keyNotAllowed', { key })
        : t('definitionEditor.diagnostics.notAllowed');
      return {
        message: rule ? `${message} ${rule}` : message,
        path: key ? [...path, key] : path,
      };
    }
    case 'additionalProperties': {
      const key = String(params.additionalProperty);
      const suggestion = closestKey(
        key,
        Object.keys(parentSchema?.properties ?? {}),
      );
      return {
        message: suggestion
          ? t('definitionEditor.diagnostics.unknownKeySuggestion', {
              key,
              suggestion,
            })
          : t('definitionEditor.diagnostics.unknownKey', { key }),
        path: [...path, key],
        fix: suggestion ? { kind: 'renameKey', to: suggestion } : undefined,
      };
    }
    case 'required': {
      const key = String(params.missingProperty);
      const node = parentSchema?.properties?.[key];
      return {
        message: t('definitionEditor.diagnostics.missingKey', { key }),
        path,
        fix: node ? { kind: 'insertKey', key, node } : undefined,
      };
    }
    case 'discriminator': {
      const variants = parentSchema?.oneOf ?? [];
      return {
        message:
          params.error === 'tag'
            ? t('definitionEditor.diagnostics.missingKey', {
                key: discriminatorKey,
              })
            : t('definitionEditor.diagnostics.expectedOneOf', {
                values: listValues(variants.map(variantTag)),
              }),
        path: params.error === 'tag' ? path : [...path, discriminatorKey],
        fix:
          params.error === 'tag'
            ? undefined
            : { kind: 'replaceValue', choices: variants.map(variantTag) },
      };
    }
    case 'oneOf': {
      const options = parentSchema?.oneOf ?? [];
      if (!options.every((option) => option.const !== undefined))
        return undefined;
      return {
        message: t('definitionEditor.diagnostics.expectedOneOf', {
          values: listValues(options.map((option) => option.const)),
        }),
        path,
        fix: {
          kind: 'replaceValue',
          choices: options.map((option) => option.const),
        },
      };
    }
    case 'enum':
      return {
        message: t('definitionEditor.diagnostics.expectedOneOf', {
          values: listValues(params.allowedValues as unknown[]),
        }),
        path,
        fix: {
          kind: 'replaceValue',
          choices: params.allowedValues as unknown[],
        },
      };
    case 'const':
      return {
        message: t('definitionEditor.diagnostics.expectedValue', {
          value: JSON.stringify(params.allowedValue),
        }),
        path,
        fix: { kind: 'replaceValue', choices: [params.allowedValue] },
      };
    case 'type':
      return {
        message: t('definitionEditor.diagnostics.expectedType', {
          type: String(params.type).replace(',null', ''),
        }),
        path,
      };
    case 'pattern': {
      if (params.pattern === codePattern)
        return { message: t('definitionEditor.diagnostics.invalidCode'), path };
      const message = t('definitionEditor.diagnostics.invalidFormat');
      return {
        message: parentSchema?.description
          ? `${message} ${parentSchema.description}`
          : message,
        path,
      };
    }
    case 'minItems':
    case 'minLength':
      return params.limit === 1
        ? { message: t('definitionEditor.diagnostics.notEmpty'), path }
        : {
            message: t('definitionEditor.diagnostics.tooShort', {
              limit: params.limit,
            }),
            path,
          };
    case 'maxItems':
    case 'maxLength':
      return {
        message: t('definitionEditor.diagnostics.tooLong', {
          limit: params.limit,
        }),
        path,
      };
    case 'minimum':
      return {
        message: t('definitionEditor.diagnostics.tooSmall', {
          limit: params.limit,
        }),
        path,
      };
    case 'maximum':
      return {
        message: t('definitionEditor.diagnostics.tooLarge', {
          limit: params.limit,
        }),
        path,
      };
    default:
      return error.message ? { message: error.message, path } : undefined;
  }
};

/** A plain starting value for an inserted key, or none for tables. */
const startingValue = (root: SchemaNode, property: SchemaNode) => {
  const node = unwrapSchema(root, property);
  if (node.const !== undefined) return node.const;
  if (node.default !== undefined && node.default !== null) return node.default;
  const choices = choicesOf(node);
  if (choices) return primaryType(node) === 'boolean' ? false : choices[0];
  switch (primaryType(node)) {
    case 'string':
      return '';
    case 'boolean':
      return false;
    case 'integer':
    case 'number':
      return 0;
    case 'array': {
      const items = node.items && unwrapSchema(root, node.items);
      return items && primaryType(items) === 'object' ? undefined : [];
    }
    default:
      return undefined;
  }
};

const lineStart = (text: string, offset: number) =>
  text.lastIndexOf('\n', offset - 1) + 1;

const lineEnd = (text: string, offset: number) => {
  const end = text.indexOf('\n', offset);
  return end < 0 ? text.length : end;
};

const isSingleLine = (text: string, { start, end }: TextRange) =>
  !text.slice(start, end).includes('\n');

/**
 * Where a new key of `mapping` goes: before the first root key for the root
 * mapping, or after a block mapping's first single-line `key: value`, at its
 * indentation. Flow mappings and mappings without one get no insertion point.
 */
const keyInsertion = (
  outline: YamlOutline,
  text: string,
  mapping: YamlPath,
) => {
  const children = outline.entries.filter(
    ({ path }) =>
      path.length === mapping.length + 1 &&
      typeof path.at(-1) === 'string' &&
      mapping.every((segment, index) => path[index] === segment),
  );
  if (!mapping.length) {
    const first = children.sort(
      (left, right) => left.range.start - right.range.start,
    )[0];
    const offset = first ? lineStart(text, first.range.start) : text.length;
    return { offset, line: (body: string) => `${body}\n` };
  }
  const collection = outline.collections.find(
    ({ kind, path }) =>
      kind === 'map' &&
      path.length === mapping.length &&
      mapping.every((segment, index) => path[index] === segment),
  );
  if (!collection || collection.flow) return undefined;
  const anchor = children.find(
    ({ valueRange }) => valueRange && isSingleLine(text, valueRange),
  );
  if (!anchor) return undefined;
  const indent = ' '.repeat(
    anchor.range.start - lineStart(text, anchor.range.start),
  );
  return {
    offset: lineEnd(text, anchor.range.start),
    line: (body: string) => `\n${indent}${body}`,
  };
};

const resolveFix = (
  root: SchemaNode,
  outline: YamlOutline,
  text: string,
  { fix, path }: Located,
): DefinitionFix | undefined => {
  const t = i18n.t.bind(i18n);
  switch (fix?.kind) {
    case 'renameKey': {
      const located = outline.entryAt(path);
      if (!located) return undefined;
      return {
        edits: [{ range: located.range, text: yamlKey(fix.to) }],
        title: t('definitionEditor.fixes.renameKey', { key: fix.to }),
      };
    }
    case 'insertKey': {
      const value = startingValue(root, fix.node);
      const insertion = keyInsertion(outline, text, path);
      if (value === undefined || !insertion) return undefined;
      const body = `${yamlKey(fix.key)}: ${yamlValue(value)}`;
      return {
        edits: [
          {
            range: { end: insertion.offset, start: insertion.offset },
            text: insertion.line(body),
          },
        ],
        title: t('definitionEditor.fixes.insertKey', { key: fix.key }),
      };
    }
    case 'replaceValue': {
      const entry = outline.entryAt(path);
      if (!entry?.valueRange || entry.value === undefined) return undefined;
      const choices = fix.choices.filter(
        (choice) => typeof choice !== 'object' && choice !== entry.value,
      );
      const choice =
        choices.length === 1
          ? choices[0]
          : closestKey(String(entry.value), choices.map(String));
      if (choice === undefined) return undefined;
      const value =
        choices.find((candidate) => String(candidate) === String(choice)) ??
        choice;
      return {
        edits: [{ range: entry.valueRange, text: yamlValue(value) }],
        title: t('definitionEditor.fixes.replaceValue', {
          value: yamlValue(value),
        }),
      };
    }
    default:
      return undefined;
  }
};

/** Errors inside a `oneOf` of constants repeat the `oneOf` error itself. */
const isNestedInConstantChoice = (error: ErrorObject, errors: ErrorObject[]) =>
  errors.some(
    (other) =>
      other !== error &&
      other.keyword === 'oneOf' &&
      other.instancePath === error.instancePath &&
      error.schemaPath.startsWith(`${other.schemaPath}/`) &&
      ((other.parentSchema as SchemaNode | undefined)?.oneOf ?? []).every(
        (option) => option.const !== undefined,
      ),
  );

const lineRange = (text: string, line: number, column: number): TextRange => {
  let start = 0;
  for (let current = 1; current < line; current++) {
    const next = text.indexOf('\n', start);
    if (next < 0) break;
    start = next + 1;
  }
  const lineEnd = text.indexOf('\n', start);
  const end = lineEnd < 0 ? text.length : lineEnd;
  return { end, start: Math.min(start + Math.max(column - 1, 0), end) };
};

/**
 * Validates a definition against its contract. The schema never rejects a
 * definition the API accepts, but the API also enforces cross-field rules, so
 * a clean result does not guarantee that saving succeeds.
 */
export const definitionDiagnostics = (
  kind: DefinitionKind,
  text: string,
): DefinitionDiagnostic[] => {
  const parsed = parseStrictYaml(text);
  if ('error' in parsed) {
    const { column, line, message, policy } = parsed.error;
    return [
      {
        message: policy
          ? i18n.t(`definitionEditor.diagnostics.${policy.code}`, {
              value: policy.value,
            })
          : i18n.t('definitionEditor.diagnostics.syntax', { message }),
        range: lineRange(text, line, column),
      },
    ];
  }
  const { data } = parsed;
  const validate = validatorFor(kind);
  if (validate(data)) return [];
  const errors = validate.errors ?? [];
  const outline = scanYaml(text);
  const root = definitionSchemas[kind];
  const diagnostics = new Map<string, DefinitionDiagnostic>();
  for (const error of errors) {
    if (isNestedInConstantChoice(error, errors)) continue;
    const located = describeError(
      error,
      pointerPath(error.instancePath, data),
      validate.schema,
    );
    if (!located) continue;
    const range = outline.rangeOf(located.path) ?? { end: 0, start: 0 };
    diagnostics.set(`${range.start}:${range.end}:${located.message}`, {
      fix: resolveFix(root, outline, text, located),
      message: located.message,
      range,
    });
  }
  return [...diagnostics.values()];
};
