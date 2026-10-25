import { useQueries, useQuery } from '@tanstack/react-query';
import { useMemo } from 'react';
import {
  checkAccess,
  maxAccessTargets,
  type AccessTargetKind,
  type Session,
} from './api';
import { authQueryKeys } from './queryKeys';
import { sessionOptions } from './queryOptions';

export type Capability = keyof NonNullable<Session['capabilities']>;

/** The permission code a capability reports, e.g. `records.write`. */
const permissionCode = (capability: Capability) =>
  capability.replace(/_([a-z]+)$/, '.$1');

const useWorkspaceCapability = (capability: Capability) =>
  useQuery({
    ...sessionOptions(),
    select: (session) =>
      session ? session.capabilities?.[capability] === true : null,
  });

/**
 * Whether the signed-in caller holds a workspace-wide capability. It is
 * `false` while the session loads, so callers that render a refusal must also
 * wait for the session query. The server still authorizes every request.
 */
export const useCan = (capability: Capability) =>
  useWorkspaceCapability(capability).data === true;

/**
 * Whether the caller holds `capability` on each of `ids`, counting grants
 * scoped to records, blueprint families and context subtrees. Targets are
 * checked with the server only when the caller lacks the workspace-wide
 * grant, and count as refused until that check answers.
 */
export const useAccess = (
  capability: Capability,
  kind: AccessTargetKind,
  ids: readonly string[],
) => {
  const workspace = useWorkspaceCapability(capability);
  const everywhere = workspace.data === true;
  const key = [...new Set(ids)].sort().join();
  const chunks = useMemo(() => {
    const unique = key ? key.split(',') : [];
    return Array.from(
      { length: Math.ceil(unique.length / maxAccessTargets) },
      (_, index) =>
        unique.slice(index * maxAccessTargets, (index + 1) * maxAccessTargets),
    );
  }, [key]);
  const permission = permissionCode(capability);
  const scoped = useQueries({
    queries: chunks.map((chunk) => ({
      queryKey: authQueryKeys.access(permission, kind, chunk),
      queryFn: () => checkAccess(permission, kind, chunk),
      enabled: workspace.data === false,
    })),
  });
  const granted = new Set(scoped.flatMap((query) => query.data?.granted ?? []));
  return {
    can: (id: string | undefined) =>
      id !== undefined && (everywhere || granted.has(id)),
    isPending:
      workspace.isPending ||
      (workspace.data === false && scoped.some((query) => query.isPending)),
  };
};

/** Whether the caller holds `capability` on one target; see `useAccess`. */
export const useCanOn = (
  capability: Capability,
  kind: AccessTargetKind,
  id: string | undefined,
) => useAccess(capability, kind, id ? [id] : []).can(id);
