/** HTTP status codes the web app acts on. */
export const httpStatus = {
  noContent: 204,
  unauthorized: 401,
  forbidden: 403,
  notFound: 404,
  conflict: 409,
} as const;

export const isSuccessStatus = (status: number) =>
  status >= 200 && status < 300;

/** A 4xx response: the request itself was refused, so repeating it will not help. */
export const isClientErrorStatus = (status: number) =>
  status >= 400 && status < 500;
