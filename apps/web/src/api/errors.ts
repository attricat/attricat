import i18n from 'i18next';
import { ApiRequestError } from './request';

/**
 * A code from `contracts/api-error-codes.json`. Both locales translate every
 * contract code under `apiErrors` (checked in `errors.test.ts`), so the
 * English keys name exactly the contract's codes.
 */
export type ApiErrorCode =
  keyof (typeof import('../i18n/locales/en.json'))['apiErrors'];

/** The error code of a missing (or unreadable) resource. */
export const notFoundErrorCode = 'not_found' satisfies ApiErrorCode;
/** A rejected request, such as a query naming fields its blueprint lacks. */
export const invalidInputErrorCode = 'invalid_input' satisfies ApiErrorCode;

/** Translation key prefix of API error codes (`apiErrors.<code>`). */
const API_ERROR_KEY_PREFIX = 'apiErrors';

/** The scalar fields of `error.details` that a translation can interpolate. */
const interpolatedDetails = (details: unknown) =>
  details && typeof details === 'object' && !Array.isArray(details)
    ? Object.fromEntries(
        Object.entries(details).filter(
          ([, value]) =>
            typeof value === 'string' ||
            typeof value === 'number' ||
            typeof value === 'boolean',
        ),
      )
    : {};

/**
 * The text for an API error reported by `code`, as in a failed request or a
 * per-item failure inside a response: translated as `apiErrors.<code>`,
 * interpolating `details`' scalar fields and the server's English `message`,
 * which is itself only the fallback for a code without a translation.
 */
export const apiErrorText = ({
  code,
  details,
  message,
}: {
  code: string;
  details?: unknown;
  message: string;
}) => {
  const key = `${API_ERROR_KEY_PREFIX}.${code}`;
  if (i18n.exists(key))
    return i18n.t(key, { ...interpolatedDetails(details), message });
  if (import.meta.env.DEV)
    console.warn(`No translation for API error code ${code}`);
  return message;
};

/**
 * The text to show for a failed request or other error; an API error with a
 * code is translated by `apiErrorText`.
 */
export function errorMessage(error: Error): string;
export function errorMessage(
  error: Error | null | undefined,
): string | undefined;
export function errorMessage(error: Error | null | undefined) {
  if (!error) return undefined;
  if (error instanceof ApiRequestError && error.code)
    return apiErrorText({
      code: error.code,
      details: error.details,
      message: error.message,
    });
  return error.message;
}
