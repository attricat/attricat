export const csrfToken = () =>
  (typeof document === 'undefined' ? '' : document.cookie)
    .split('; ')
    .find((cookie) => cookie.startsWith('catalog_csrf='))
    ?.split('=')[1];

/** Sends same-origin cookies and the synchronizer token for unsafe requests. */
export const apiFetch = (path: string, ...args: [RequestInit?]) => {
  const init = args[0];
  const method = (init?.method ?? 'GET').toUpperCase();
  const csrf = !['GET', 'HEAD', 'OPTIONS'].includes(method) ? csrfToken() : '';
  if (!csrf) return fetch(path, ...args);

  const headers = new Headers(init?.headers);
  headers.set('X-Catalog-Csrf', csrf);
  return fetch(path, { ...init, headers });
};
