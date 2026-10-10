import { z } from 'zod';
import i18n from 'i18next';
import { apiFetch, csrfToken } from './fetch';

export const apiErrorSchema = z.object({
  error: z.object({
    code: z.string(),
    message: z.string(),
    details: z.unknown().optional(),
  }),
});

export class ApiRequestError extends Error {
  status: number;
  code?: string;
  /** Endpoint-specific `error.details`; validate it at the feature boundary. */
  details?: unknown;

  constructor(
    status: number,
    message: string,
    code?: string,
    details?: unknown,
  ) {
    super(message);
    this.name = 'ApiRequestError';
    this.status = status;
    this.code = code;
    this.details = details;
  }
}

export const apiRequestError = (
  status: number,
  body: unknown,
  fallbackMessage = i18n.t('errors.requestFailed', { status }),
) => {
  const error = apiErrorSchema.safeParse(body);
  return new ApiRequestError(
    status,
    error.success ? error.data.error.message : fallbackMessage,
    error.success ? error.data.error.code : undefined,
    error.success ? error.data.error.details : undefined,
  );
};

export const responseError = async (response: Response) => {
  const body =
    typeof response.json === 'function'
      ? await response.json().catch(() => undefined)
      : undefined;
  return apiRequestError(response.status, body);
};

export const request = async <T>(
  path: string,
  schema: z.ZodType<T>,
  init?: RequestInit,
): Promise<T> => {
  const response = await responseFor(path, init);
  const result = schema.safeParse(
    response.status === 204 ? undefined : await response.json(),
  );
  if (!result.success)
    throw new Error(
      i18n.t('errors.invalidApiResponse', {
        details: z.prettifyError(result.error),
      }),
    );
  return result.data;
};

const responseFor = async (path: string, init?: RequestInit) => {
  const response = await (init === undefined
    ? apiFetch(path)
    : apiFetch(path, init));
  if (!response.ok) throw await responseError(response);
  return response;
};

export const requestUpload = async <T>(
  path: string,
  data: FormData,
  schema: z.ZodType<T>,
  onProgress?: (progress: number) => void,
  method: 'POST' | 'PUT' = 'POST',
): Promise<T> => {
  if (typeof XMLHttpRequest === 'undefined') {
    const result = await request(path, schema, { method, body: data });
    onProgress?.(100);
    return result;
  }
  return new Promise<T>((resolve, reject) => {
    const upload = new XMLHttpRequest();
    upload.open(method, path);
    upload.withCredentials = true;
    const csrf = csrfToken();
    if (csrf) upload.setRequestHeader('X-Attricat-Csrf', csrf);
    upload.upload.onprogress = (event) => {
      if (event.lengthComputable)
        onProgress?.(Math.round((event.loaded / event.total) * 100));
    };
    upload.onerror = () =>
      reject(apiRequestError(0, undefined, i18n.t('errors.uploadFailed')));
    upload.onload = () => {
      let body: unknown;
      try {
        body = JSON.parse(upload.responseText);
      } catch {
        body = undefined;
      }
      if (upload.status < 200 || upload.status >= 300) {
        reject(apiRequestError(upload.status, body));
        return;
      }
      const result = schema.safeParse(body);
      if (!result.success) {
        reject(
          new Error(
            i18n.t('errors.invalidApiResponse', {
              details: z.prettifyError(result.error),
            }),
          ),
        );
        return;
      }
      onProgress?.(100);
      resolve(result.data);
    };
    upload.send(data);
  });
};

export const requestText = async (path: string, init?: RequestInit) =>
  (await responseFor(path, init)).text();

export const requestNoContent = async (path: string, init?: RequestInit) => {
  await responseFor(path, init);
};
