export const csrfToken = () =>
  (typeof document === 'undefined' ? '' : document.cookie)
    .split('; ')
    .find((cookie) => cookie.startsWith('catalog_csrf='))
    ?.split('=')[1];

/** Sends same-origin cookies and the synchronizer token for unsafe requests. */
export const apiFetch = (path: string, init: RequestInit = {}) => {
  const method = (init.method ?? 'GET').toUpperCase();
  const headers = new Headers(init.headers);
  if (!['GET', 'HEAD', 'OPTIONS'].includes(method)) {
    const csrf = csrfToken();
    if (csrf) headers.set('X-Catalog-Csrf', csrf);
  }
  return fetch(path, { ...init, credentials: 'same-origin', headers });
};
