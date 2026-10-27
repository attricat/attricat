import { z } from 'zod';
import type { GrantKind } from './constants';

export type ExtensionGrant = { grant_kind: GrantKind; grant_id: string };
export type RequestedGrant = { kind: GrantKind; id: string };

/** The parts of a manifest that request grants; the server validated the rest. */
const manifestPermissionsSchema = z.looseObject({
  permissions: z.array(z.string()).optional(),
  optional_permissions: z.array(z.string()).optional(),
  host_permissions: z.array(z.looseObject({ id: z.string() })).optional(),
  optional_host_permissions: z
    .array(z.looseObject({ id: z.string() }))
    .optional(),
  event_contracts: z
    .looseObject({
      exports: z.array(z.looseObject({ id: z.string() })).optional(),
      consumes: z
        .array(z.looseObject({ provider: z.string(), contract: z.string() }))
        .optional(),
    })
    .optional(),
});

export const grantKey = (kind: GrantKind, id: string) => `${kind}:${id}`;

/** Lists every distinct grant the manifest requests, required and optional alike. */
const requestedGrants = (manifest: unknown): RequestedGrant[] => {
  const parsed = manifestPermissionsSchema.safeParse(manifest);
  if (!parsed.success) return [];
  const value = parsed.data;
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
