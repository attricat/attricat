import { recordServerTiming } from '../features/inspector/timing';

/** The cookie holding the synchronizer token the server issued. */
const csrfCookie = 'attricat_csrf';
/** The header that echoes the token on requests that change state. */
export const csrfHeader = 'X-Attricat-Csrf';
/** Methods that change nothing, so they need no token. */
const safeMethods = new Set(['GET', 'HEAD', 'OPTIONS']);

export const csrfToken = () =>
  (typeof document === 'undefined' ? '' : document.cookie)
    .split('; ')
    .find((cookie) => cookie.startsWith(`${csrfCookie}=`))
    ?.split('=')[1];

/** Sends same-origin cookies and the synchronizer token for unsafe requests. */
export const apiFetch = (path: string, ...args: [RequestInit?]) => {
  const init = args[0];
  const method = (init?.method ?? 'GET').toUpperCase();
  const csrf = safeMethods.has(method) ? '' : csrfToken();
  const response = csrf
    ? (() => {
        const headers = new Headers(init?.headers);
        headers.set(csrfHeader, csrf);
        return fetch(path, { ...init, headers });
      })()
    : fetch(path, ...args);
  return response.then((result) => {
    recordServerTiming(result.headers?.get('server-timing') ?? null);
    return result;
  });
};
