import type { GrantKind } from './constants';

export type ExtensionGrant = { grant_kind: GrantKind; grant_id: string };
export type RequestedGrant = { kind: GrantKind; id: string };

type ManifestPermissions = {
  permissions?: string[];
  optional_permissions?: string[];
  host_permissions?: { id: string }[];
  optional_host_permissions?: { id: string }[];
  event_contracts?: {
    exports?: { id: string }[];
    consumes?: { provider: string; contract: string }[];
  };
};

export const grantKey = (kind: GrantKind, id: string) => `${kind}:${id}`;

/** Lists every distinct grant the manifest requests, required and optional alike. */
export const requestedGrants = (manifest: unknown): RequestedGrant[] => {
  const value = manifest as ManifestPermissions;
  const requested: RequestedGrant[] = [
    ...[
      ...(value.permissions ?? []),
      ...(value.optional_permissions ?? []),
    ].map((id) => ({ kind: 'capability' as const, id })),
    ...[
      ...(value.host_permissions ?? []),
      ...(value.optional_host_permissions ?? []),
    ].map((item) => ({ kind: 'host_permission' as const, id: item.id })),
    ...(value.event_contracts?.exports ?? []).map((item) => ({
      kind: 'event_publish' as const,
      id: item.id,
    })),
    ...(value.event_contracts?.consumes ?? []).map((item) => ({
      kind: 'event_subscribe' as const,
      id: `${item.provider}:${item.contract}`,
    })),
  ];
  return [
    ...new Map(
      requested.map((item) => [grantKey(item.kind, item.id), item]),
    ).values(),
  ];
};

/** Requested grants that have not been granted yet. */
export const pendingGrants = (manifest: unknown, grants: ExtensionGrant[]) => {
  const granted = new Set(
    grants.map((item) => grantKey(item.grant_kind, item.grant_id)),
  );
  return requestedGrants(manifest).filter(
    (item) => !granted.has(grantKey(item.kind, item.id)),
  );
};
