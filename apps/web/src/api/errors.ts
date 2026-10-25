import i18n from 'i18next';
import { ApiRequestError } from './request';

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
 * The text to show for a failed request or other error. An API error is
 * translated by its code as `apiErrors.<code>`, interpolating its scalar
 * details and the server's English `message`; the message itself is only the
 * fallback for a code without a translation.
 */
export const errorMessage = (error: unknown): string | undefined => {
  if (error === null || error === undefined) return undefined;
  if (error instanceof ApiRequestError && error.code) {
    const key = `${API_ERROR_KEY_PREFIX}.${error.code}`;
    if (i18n.exists(key))
      return i18n.t(key, {
        ...interpolatedDetails(error.details),
        message: error.message,
      });
    if (import.meta.env.DEV)
      console.warn(`No translation for API error code ${error.code}`);
  }
  return error instanceof Error ? error.message : String(error);
};
