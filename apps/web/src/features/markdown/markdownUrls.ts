/**
 * Allow HTTP(S), mailto, relative paths and fragments. Whitespace, control
 * characters and backslashes are rejected outright because browsers strip or
 * reinterpret them, which could turn a harmless-looking link into another
 * scheme or a protocol-relative URL.
 */
export const markdownUrlTransform = (url: string): string => {
  if (/[\s\p{Cc}\\]/u.test(url) || url.startsWith('//')) return '';
  return /^(?:https?:|mailto:|[^:]*$)/i.test(url) ? url : '';
};
