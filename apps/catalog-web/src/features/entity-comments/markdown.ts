// Do not permit executable schemes, protocol-relative URLs, or URL whitespace.
export const safeCommentUrl = (url: string) => {
  if (/[\\\s]/u.test(url)) return '';
  if (/^(https?:\/\/|mailto:)/i.test(url)) return url;
  if (url.startsWith('/') && !url.startsWith('//')) return url;
  if (url.startsWith('#')) return url;
  return '';
};
