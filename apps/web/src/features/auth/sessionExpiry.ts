import type { QueryClient } from '@tanstack/react-query';
import { ApiRequestError } from '../../api/request';
import { httpStatus } from '../../api/httpStatus';
import { authQueryKeys } from './queryKeys';

/** The API's code for a request without a valid session or token. */
export const unauthenticatedErrorCode = 'unauthenticated';

/**
 * Rechecks the session when a request reports it is no longer signed in, so
 * an expired or revoked session leads to sign-in instead of leaving every
 * page saying it cannot load.
 */
export const recheckSessionWhenSignedOut =
  (client: QueryClient) => (error: unknown) => {
    if (
      error instanceof ApiRequestError &&
      error.status === httpStatus.unauthorized &&
      error.code === unauthenticatedErrorCode
    )
      void client.invalidateQueries({ queryKey: authQueryKeys.session() });
  };
