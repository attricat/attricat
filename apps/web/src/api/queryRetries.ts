import { z } from 'zod';
import { httpStatus, isClientErrorStatus } from './httpStatus';
import { ApiRequestError } from './request';

/** React Query's default retry count for unexpected failures. */
export const defaultQueryRetries = 3;

/**
 * Retries a failed read unless the request itself was refused, such as a
 * missing record, or a malformed ID or response failed its schema; repeating
 * will not change either. A rate limit is temporary, so it is retried.
 */
export const retryUnlessRefused = (failureCount: number, error: Error) =>
  !(error instanceof z.ZodError) &&
  !(
    error instanceof ApiRequestError &&
    isClientErrorStatus(error.status) &&
    error.status !== httpStatus.tooManyRequests
  ) &&
  failureCount < defaultQueryRetries;
