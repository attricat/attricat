import Ajv, { type ErrorObject } from 'ajv';
import i18n from '../../i18n';
import { lexiconText } from '../lexicon/lexicon';
import { isEmailAddress } from '../views/controls/email';
import type { JsonSchema } from './schemas';

/** Describes a value schema's `pattern` in words, for hints and errors. */
const PATTERN_HINT_SCHEMA_KEY = 'x-attricat-pattern-hint';

// Verbose errors carry the subschema that failed, so a pattern's hint is
// found even inside `anyOf` or `allOf`.
const ajv = new Ajv({ strict: false, verbose: true });
ajv.addFormat('email', isEmailAddress);

export const validatesJsonSchema = (
  value: unknown,
  schema: JsonSchema | null | undefined,
) => {
  return jsonSchemaValidationMessage(value, schema) === undefined;
};

export const jsonSchemaValidationErrors = (
  value: unknown,
  schema: JsonSchema | null | undefined,
): ErrorObject[] | undefined => {
  if (schema == null) return [];

  try {
    if (ajv.validate(schema, value)) return [];
    return ajv.errors ?? [];
  } catch {
    return undefined;
  }
};

const jsonSchemaValidationMessage = (
  value: unknown,
  schema: JsonSchema | null | undefined,
): string | undefined => {
  const errors = jsonSchemaValidationErrors(value, schema);
  if (errors === undefined) return i18n.t('records.schemaValidationFailed');
  return errors[0]?.message;
};

/** The text of a schema's pattern hint, with lexicon references resolved. */
export const patternHint = (schema: unknown): string | undefined => {
  if (!schema || typeof schema !== 'object') return undefined;
  const hint = (schema as Record<string, unknown>)[PATTERN_HINT_SCHEMA_KEY];
  return typeof hint === 'string' && hint.trim()
    ? lexiconText(hint.trim())
    : undefined;
};

const formatValue = (value: unknown) =>
  typeof value === 'number'
    ? new Intl.NumberFormat(i18n.language).format(value)
    : typeof value === 'string'
      ? value
      : JSON.stringify(value);

const keywordMessage = (error: ErrorObject): string | undefined => {
  const { params } = error;
  switch (error.keyword) {
    case 'minLength':
    case 'maxLength':
      return i18n.t(`records.schemaErrors.${error.keyword}`, {
        count: Number(params.limit),
      });
    case 'minimum':
    case 'maximum':
    case 'exclusiveMinimum':
    case 'exclusiveMaximum':
      return i18n.t(`records.schemaErrors.${error.keyword}`, {
        limit: formatValue(params.limit),
      });
    case 'multipleOf':
      return i18n.t('records.schemaErrors.multipleOf', {
        step: formatValue(params.multipleOf),
      });
    case 'pattern': {
      const hint = patternHint(error.parentSchema);
      return hint
        ? i18n.t('records.schemaErrors.patternWithHint', { hint })
        : i18n.t('records.schemaErrors.pattern');
    }
    case 'format':
      return params.format === 'email'
        ? i18n.t('records.schemaErrors.email')
        : i18n.t('records.schemaErrors.format');
    case 'enum':
      return i18n.t('records.schemaErrors.enum');
    case 'const':
      return i18n.t('records.schemaErrors.const', {
        value: formatValue(params.allowedValue),
      });
    default:
      return undefined;
  }
};

/**
 * What a value must change to meet its schema, in words: the first failed
 * rule that has a message. Composite keywords such as `anyOf` report the
 * rules inside them first, so those are described when they can be.
 */
export const jsonSchemaErrorMessage = (
  errors: readonly ErrorObject[],
): string | undefined => {
  for (const error of errors) {
    const message = keywordMessage(error);
    if (message) return message;
  }
  return undefined;
};
