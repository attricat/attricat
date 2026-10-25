import { queryOptions } from '@tanstack/react-query';
import { currentSession } from './api';
import { authQueryKeys } from './queryKeys';

/** The signed-in session, or `null` when nobody is signed in. */
export const sessionOptions = () =>
  queryOptions({
    queryKey: authQueryKeys.session(),
    queryFn: currentSession,
  });
