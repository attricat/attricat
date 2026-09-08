import { z } from 'zod';
import { apiFetch } from '../features/auth/request';

export const apiErrorSchema = z.object({
  error: z.object({ code: z.string(), message: z.string() }),
});

export class ApiRequestError extends Error {
  status: number;
  code?: string;

  constructor(status: number, message: string, code?: string) {
    super(message);
    this.name = 'ApiRequestError';
    this.status = status;
    this.code = code;
  }
}

export const apiRequestError = (
  status: number,
  body: unknown,
  fallbackMessage = `Request failed (${status})`,
) => {
  const error = apiErrorSchema.safeParse(body);
  return new ApiRequestError(
    status,
    error.success ? error.data.error.message : fallbackMessage,
    error.success ? error.data.error.code : undefined,
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
  const response = await (init === undefined
    ? apiFetch(path)
    : apiFetch(path, init));
  if (!response.ok) throw await responseError(response);

  const result = schema.safeParse(
    response.status === 204 ? undefined : await response.json(),
  );
  if (!result.success)
    throw new Error(`Invalid API response: ${z.prettifyError(result.error)}`);
  return result.data;
};

export const requestNoContent = async (path: string, init?: RequestInit) => {
  const response = await (init === undefined
    ? apiFetch(path)
    : apiFetch(path, init));
  if (!response.ok) throw await responseError(response);
};
