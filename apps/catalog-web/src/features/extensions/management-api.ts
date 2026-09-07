import { z } from 'zod';
import { apiFetch } from '../auth/request';

const jsonValue: z.ZodType<unknown> = z.lazy(() =>
  z.union([
    z.string(),
    z.number(),
    z.boolean(),
    z.null(),
    z.array(jsonValue),
    z.record(z.string(), jsonValue),
  ]),
);
const errorSchema = z.object({ error: z.object({ message: z.string() }) });
const grantSchema = z.object({
  grant_kind: z.enum([
    'capability',
    'host_permission',
    'event_publish',
    'event_subscribe',
  ]),
  grant_id: z.string(),
  granted_at: z.string(),
});
const lifecycleSchema = z.object({
  id: z.uuid(),
  operation: z.string(),
  prior_state: z.string().nullable(),
  new_state: z.string().nullable(),
  outcome: z.string(),
  actor_user_id: z.uuid().nullable(),
  actor_token_id: z.uuid().nullable(),
  source: z.string().nullable(),
  diagnostics: jsonValue,
  created_at: z.string(),
});
export const installationSchema = z.object({
  id: z.uuid(),
  extension_id: z.string(),
  installed_release_id: z.uuid(),
  state: z.enum(['disabled', 'enabled', 'quarantined']),
  configuration: jsonValue,
  configuration_version: z.number().int().nullable(),
  created_at: z.string(),
  updated_at: z.string(),
  version: z.string(),
  manifest: jsonValue,
  manifest_sha256: z.string(),
  source: z.string(),
});
const detailSchema = z.object({
  installation: installationSchema,
  grants: z.array(grantSchema),
  lifecycle: z.array(lifecycleSchema),
});
const discoveredSchema = z.object({
  registry_source: z.string(),
  id: z.string(),
  repository: z.string(),
  name: z.string(),
  description: z.string(),
  icon: z.string().nullable(),
});
const releaseSchema = z.object({
  source: z.string(),
  release_id: z.number().int(),
  tag_name: z.string(),
  name: z.string(),
  published_at: z.string().nullable(),
  asset: z.object({
    id: z.number().int(),
    name: z.string(),
    download_url: z.string(),
  }),
});
const registryDetailsSchema = z.object({
  extension: discoveredSchema,
  readme: z.string(),
  releases: z.array(releaseSchema),
});
export type ExtensionInstallation = z.infer<typeof installationSchema>;
export type ExtensionDetail = z.infer<typeof detailSchema>;
export type DiscoveredExtension = z.infer<typeof discoveredSchema>;
export type RegistryDetails = z.infer<typeof registryDetailsSchema>;

const request = async <T>(
  path: string,
  schema: z.ZodType<T>,
  init?: RequestInit,
) => {
  const response = await apiFetch(path, init);
  if (!response.ok) {
    const parsed = errorSchema.safeParse(
      await response.json().catch(() => null),
    );
    throw new Error(
      parsed.success
        ? parsed.data.error.message
        : `Request failed (${response.status})`,
    );
  }
  return schema.parse(await response.json());
};
const noContent = async (path: string, init: RequestInit) => {
  const response = await apiFetch(path, init);
  if (!response.ok) throw new Error(`Request failed (${response.status})`);
};
const body = (value: unknown, method = 'POST'): RequestInit => ({
  method,
  headers: { 'Content-Type': 'application/json' },
  body: JSON.stringify(value),
});

export const discoverExtensions = () =>
  request('/api/extension-registries/discover', z.array(discoveredSchema));
export const registryDetails = (owner: string, repository: string) =>
  request(
    `/api/extension-registries/extensions/${encodeURIComponent(owner)}/${encodeURIComponent(repository)}`,
    registryDetailsSchema,
  );
export const installedExtensions = () =>
  request('/api/extensions', z.array(installationSchema));
export const extensionDetail = (id: string) =>
  request(`/api/extensions/${encodeURIComponent(id)}`, detailSchema);
export const installExtension = (input: {
  owner: string;
  repository: string;
  release_id: number;
}) => request('/api/extensions', installationSchema, body(input));
export const sideloadExtension = (archive: File) =>
  request('/api/extensions/sideload', installationSchema, {
    method: 'POST',
    headers: { 'Content-Type': 'application/zstd' },
    body: archive,
  });
export const upgradeExtension = (
  id: string,
  input: { owner: string; repository: string; release_id: number },
) =>
  request(
    `/api/extensions/${encodeURIComponent(id)}/upgrade`,
    installationSchema,
    body(input),
  );
export const configureExtension = (id: string, configuration: unknown) =>
  request(
    `/api/extensions/${encodeURIComponent(id)}/configure`,
    installationSchema,
    body({ configuration }, 'PUT'),
  );
export const grantExtension = (
  id: string,
  grant_kind:
    'capability' | 'host_permission' | 'event_publish' | 'event_subscribe',
  grant_id: string,
) =>
  noContent(
    `/api/extensions/${encodeURIComponent(id)}/grants`,
    body({ grant_kind, grant_id }),
  );
export const revokeExtensionGrant = (id: string, kind: string, grant: string) =>
  noContent(
    `/api/extensions/${encodeURIComponent(id)}/grants/${encodeURIComponent(kind)}/${encodeURIComponent(grant)}`,
    { method: 'DELETE' },
  );
export const lifecycleExtension = (
  id: string,
  action: 'enable' | 'disable' | 'quarantine',
) =>
  request(
    `/api/extensions/${encodeURIComponent(id)}/${action}`,
    installationSchema,
    action === 'quarantine'
      ? body({ diagnostic_code: 'manual_quarantine' })
      : { method: 'POST' },
  );
export const removeExtension = (id: string) =>
  noContent(`/api/extensions/${encodeURIComponent(id)}`, { method: 'DELETE' });
