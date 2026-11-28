import i18n from 'i18next';
import { z } from 'zod';
import { httpStatus } from './httpStatus';
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
/**
 * The failure for a resource that cannot exist, such as one named by a
 * malformed ID, reported as the API reports a missing one.
 */
export const notFoundError = () =>
  new ApiRequestError(
    httpStatus.notFound,
    'resource was not found',
    notFoundErrorCode,
  );
/** An extension command refused the request with a message for its user. */
export const extensionCommandRejectedErrorCode =
  'extension_command_rejected' satisfies ApiErrorCode;
/** A rejected request, such as a query naming fields its blueprint lacks. */
export const invalidInputErrorCode = 'invalid_input' satisfies ApiErrorCode;

/** Translation key prefix of API error codes (`apiErrors.<code>`). */
const API_ERROR_KEY_PREFIX = 'apiErrors';
/**
 * Translation key prefix of a code's message for one resource, such as a
 * `code_taken` whose `details.resource` names what holds the code
 * (`apiErrorResources.<code>.<resource>`). Without one, the code's own
 * message is used.
 */
const API_ERROR_RESOURCE_KEY_PREFIX = 'apiErrorResources';

/** `details.resource` of an API error, when it names one. */
const detailsResource = (details: unknown) =>
  details &&
  typeof details === 'object' &&
  'resource' in details &&
  typeof details.resource === 'string'
    ? details.resource
    : null;

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
  const options = { ...interpolatedDetails(details), message };
  const resource = detailsResource(details);
  const resourceKey = `${API_ERROR_RESOURCE_KEY_PREFIX}.${code}.${resource}`;
  if (resource && i18n.exists(resourceKey)) return i18n.t(resourceKey, options);
  const key = `${API_ERROR_KEY_PREFIX}.${code}`;
  if (i18n.exists(key)) return i18n.t(key, options);
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
  // A schema failure's own message is its issues as JSON; show their text.
  if (error instanceof z.ZodError)
    return [...new Set(error.issues.map((issue) => issue.message))].join(' ');
  return error.message;
}
