import Ajv2020, {
  type ErrorObject,
  type ValidateFunction,
} from 'ajv/dist/2020';
import { parse, TomlError } from 'smol-toml';
import i18n from '../../i18n';
import { discriminatorKey, type DefinitionKind } from './constants';
import {
  definitionSchemas,
  taggedVariants,
  variantTag,
  type SchemaNode,
} from './definitionSchema';
import { scanToml, type TextRange, type TomlPath } from './tomlOutline';

export type DefinitionDiagnostic = {
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
  return taggedVariants(root, node as SchemaNode)
    ? { ...copy, discriminator: { propertyName: discriminatorKey } }
    : copy;
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

/** Converts a JSON pointer into a TOML path, using the data to type indices. */
const pointerPath = (pointer: string, data: unknown): TomlPath => {
  const path: TomlPath = [];
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

type Located = { message: string; path: TomlPath };

const describeError = (
  error: ErrorObject,
  path: TomlPath,
): Located | undefined => {
  const t = i18n.t.bind(i18n);
  const params = error.params as Record<string, unknown>;
  const parentSchema = error.parentSchema as SchemaNode | undefined;
  switch (error.keyword) {
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
      };
    }
    case 'required':
      return {
        message: t('definitionEditor.diagnostics.missingKey', {
          key: params.missingProperty,
        }),
        path,
      };
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
      };
    }
    case 'enum':
      return {
        message: t('definitionEditor.diagnostics.expectedOneOf', {
          values: listValues(params.allowedValues as unknown[]),
        }),
        path,
      };
    case 'const':
      return {
        message: t('definitionEditor.diagnostics.expectedValue', {
          value: JSON.stringify(params.allowedValue),
        }),
        path,
      };
    case 'type':
      return {
        message: t('definitionEditor.diagnostics.expectedType', {
          type: String(params.type).replace(',null', ''),
        }),
        path,
      };
    case 'pattern':
      return {
        message: t('definitionEditor.diagnostics.invalidCode'),
        path,
      };
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
  let data: unknown;
  try {
    data = parse(text);
  } catch (error) {
    if (!(error instanceof TomlError)) throw error;
    const [summary] = error.message.split('\n');
    return [
      {
        message: i18n.t('definitionEditor.diagnostics.syntax', {
          message: summary,
        }),
        range: lineRange(text, error.line, error.column),
      },
    ];
  }
  const validate = validatorFor(kind);
  if (validate(data)) return [];
  const errors = validate.errors ?? [];
  const outline = scanToml(text);
  const diagnostics = new Map<string, DefinitionDiagnostic>();
  for (const error of errors) {
    if (isNestedInConstantChoice(error, errors)) continue;
    const located = describeError(error, pointerPath(error.instancePath, data));
    if (!located) continue;
    const range = outline.rangeOf(located.path) ?? { end: 0, start: 0 };
    diagnostics.set(`${range.start}:${range.end}:${located.message}`, {
      message: located.message,
      range,
    });
  }
  return [...diagnostics.values()];
};
