import i18n from 'i18next';
import { z } from 'zod';

type Issue = Parameters<NonNullable<z.core.$ZodConfig['customError']>>[0];

const bound = (value: unknown) => Number(value);

/**
 * Words for the input limits forms enforce, in the user's language. Zod's own
 * messages describe the schema, such as "Too small: expected array to have
 * >=1 items". A message a schema sets itself takes precedence over these, and
 * other failures, such as a response of the wrong shape, keep Zod's text.
 */
export const zodErrorMessage = (issue: Issue): string | undefined => {
  const t = i18n.t.bind(i18n);
  if (issue.code === 'too_small') {
    const min = bound(issue.minimum);
    if (issue.origin === 'string')
      return min <= 1
        ? t('validation.required')
        : t('validation.tooShort', { count: min });
    if (issue.origin === 'array' || issue.origin === 'set')
      return t('validation.chooseAtLeast', { count: min });
    if (issue.origin === 'number' || issue.origin === 'int')
      return t('validation.atLeast', { min });
  }
  if (issue.code === 'too_big') {
    const max = bound(issue.maximum);
    if (issue.origin === 'string')
      return t('validation.tooLong', { count: max });
    if (issue.origin === 'array' || issue.origin === 'set')
      return t('validation.chooseAtMost', { count: max });
    if (issue.origin === 'number' || issue.origin === 'int')
      return t('validation.atMost', { max });
  }
  if (issue.code === 'invalid_format' && issue.format === 'email')
    return t('validation.email');
  return undefined;
};

z.config({ customError: zodErrorMessage });
