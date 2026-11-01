import { z } from 'zod';
import { httpStatus, isClientErrorStatus } from './httpStatus';
import { ApiRequestError } from './request';

/** React Query's default retry count for unexpected failures. */
export const defaultQueryRetries = 3;

/**
 * Whether the API refused the request itself, such as a missing record or a
 * malformed ID, rather than failing to answer it. A rate limit is temporary,
 * so it is not a refusal.
 */
export const isRefusedRequest = (error: Error) =>
  error instanceof ApiRequestError &&
  isClientErrorStatus(error.status) &&
  error.status !== httpStatus.tooManyRequests;

/**
 * Retries a failed read unless the request was refused or its response
 * failed its schema; repeating will not change either.
 */
export const retryUnlessRefused = (failureCount: number, error: Error) =>
  !(error instanceof z.ZodError) &&
  !isRefusedRequest(error) &&
  failureCount < defaultQueryRetries;
