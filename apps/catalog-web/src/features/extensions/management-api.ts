import { z } from 'zod';
import { request, requestNoContent } from '../../api/request';
import {
  detailSchema,
  discoveredSchema,
  installationSchema,
  registryDetailsSchema,
} from './schemas';

export {
  installationSchema,
  type DiscoveredExtension,
  type ExtensionDetail,
  type ExtensionInstallation,
  type RegistryDetails,
} from './schemas';

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
  requestNoContent(
    `/api/extensions/${encodeURIComponent(id)}/grants`,
    body({ grant_kind, grant_id }),
  );
export const revokeExtensionGrant = (id: string, kind: string, grant: string) =>
  requestNoContent(
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
  requestNoContent(`/api/extensions/${encodeURIComponent(id)}`, {
    method: 'DELETE',
  });
