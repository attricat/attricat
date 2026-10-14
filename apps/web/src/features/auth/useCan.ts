import { useQuery } from '@tanstack/react-query';
import { currentSession, type Session } from './api';
import { authQueryKeys } from './queryKeys';

export type Capability = keyof NonNullable<Session['capabilities']>;

/**
 * Whether the signed-in caller holds a workspace-wide capability. It is
 * `false` while the session loads, so callers that render a refusal must also
 * wait for the session query. The server still authorizes every request.
 */
export const useCan = (capability: Capability) =>
  useQuery({
    queryKey: authQueryKeys.session(),
    queryFn: currentSession,
    select: (session) => session?.capabilities?.[capability] === true,
  }).data === true;
