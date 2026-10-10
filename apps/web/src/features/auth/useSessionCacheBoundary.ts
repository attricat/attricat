import { useQueryClient } from '@tanstack/react-query';
import { useLayoutEffect, useState } from 'react';
import type { Session } from './api';
import { authQueryKeys } from './queryKeys';

/** Hide protected children until caches from the previous identity are gone. */
export const useSessionCacheBoundary = (
  session: Session | null | undefined,
  settled: boolean,
) => {
  const client = useQueryClient();
  const identity = session
    ? JSON.stringify([session.workspace_id, session.user_id])
    : null;
  const [cacheIdentity, setCacheIdentity] = useState<string | null>();
  const ready = settled && cacheIdentity === identity;

  useLayoutEffect(() => {
    if (!settled || cacheIdentity === identity) return;
    // Removing queries also cancels in-flight work. Keep the session observer
    // alive, but never let a new account reuse protected data or mutations.
    const sessionQuery = client.getQueryCache().find({
      queryKey: authQueryKeys.session(),
      exact: true,
    });
    client.removeQueries({ predicate: (query) => query !== sessionQuery });
    client.getMutationCache().clear();
    // This layout effect synchronizes an external cache before children mount.
    // eslint-disable-next-line react-hooks/set-state-in-effect
    setCacheIdentity(identity);
  }, [cacheIdentity, client, identity, settled]);

  return { identity, ready };
};
