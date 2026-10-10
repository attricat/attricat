import i18n from '../../../i18n';

/** Navigation policy, independent of editor and server-side schema validation. */
export const safeUrl = (value: unknown): string | undefined => {
  if (typeof value !== 'string' || !/^https?:\/\/[^/?#]/i.test(value))
    return undefined;
  // Do not let URL's permissive parser silently strip whitespace or reinterpret backslashes.
  if (/[\s\p{Cc}\\]/u.test(value)) return undefined;
  try {
    const url = new URL(value);
    if (!url.hostname || url.username || url.password) return undefined;
    return url.href;
  } catch {
    return undefined;
  }
};

export const validateUrl = (value: string) =>
  !value || safeUrl(value) ? undefined : i18n.t('views.urlInvalid');
