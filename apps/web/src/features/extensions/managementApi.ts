import { z } from 'zod';
import { request, requestNoContent, jsonRequest } from '../../api/request';
import { manualQuarantineDiagnosticCode, type GrantKind } from './constants';
import {
  detailSchema,
  discoveredSchema,
  installationSchema,
  registryDetailsSchema,
  workspaceExtensionLayoutSchema,
  type WorkspaceExtensionLayout,
} from './schemas';

export type { ExtensionDetail, ExtensionInstallation } from './schemas';

export const discoverExtensions = () =>
  request('/api/extension-registries/discover', z.array(discoveredSchema));
export const registryDetails = (owner: string, repository: string) =>
  request(
    `/api/extension-registries/extensions/${encodeURIComponent(owner)}/${encodeURIComponent(repository)}`,
    registryDetailsSchema,
  );
export const installedExtensions = () =>
  request('/api/extensions', z.array(installationSchema));
export const workspaceExtensionLayout = () =>
  request('/api/workspace/extension-layout', workspaceExtensionLayoutSchema);
export const updateWorkspaceExtensionLayout = (
  layout: WorkspaceExtensionLayout,
) =>
  requestNoContent(
    '/api/workspace/extension-layout',
    jsonRequest('PUT', layout),
  );
export const extensionDetail = (id: string) =>
  request(`/api/extensions/${encodeURIComponent(id)}`, detailSchema);
export const installExtension = (input: ExtensionReleaseReference) =>
  request('/api/extensions', installationSchema, jsonRequest('POST', input));
export const sideloadExtension = (archive: File) =>
  request('/api/extensions/sideload', installationSchema, {
    method: 'POST',
    headers: { 'Content-Type': 'application/zstd' },
    body: archive,
  });
export type ExtensionReleaseReference = {
  owner: string;
  repository: string;
  release_id: number;
};

export type ExtensionLifecycleAction = 'enable' | 'disable' | 'quarantine';

export const upgradeExtension = (
  id: string,
  input: ExtensionReleaseReference,
) =>
  request(
    `/api/extensions/${encodeURIComponent(id)}/upgrade`,
    installationSchema,
    jsonRequest('POST', input),
  );
export const configureExtension = (id: string, configuration: unknown) =>
  request(
    `/api/extensions/${encodeURIComponent(id)}/configure`,
    installationSchema,
    jsonRequest('PUT', { configuration }),
  );
export const grantExtension = (
  id: string,
  grant_kind: GrantKind,
  grant_id: string,
) =>
  requestNoContent(
    `/api/extensions/${encodeURIComponent(id)}/grants`,
    jsonRequest('POST', { grant_kind, grant_id }),
  );
export const revokeExtensionGrant = (
  id: string,
  kind: GrantKind,
  grant: string,
) =>
  requestNoContent(
    `/api/extensions/${encodeURIComponent(id)}/grants/${encodeURIComponent(kind)}/${encodeURIComponent(grant)}`,
    { method: 'DELETE' },
  );
export const lifecycleExtension = (
  id: string,
  action: ExtensionLifecycleAction,
) =>
  request(
    `/api/extensions/${encodeURIComponent(id)}/${action}`,
    installationSchema,
    action === 'quarantine'
      ? jsonRequest('POST', { diagnostic_code: manualQuarantineDiagnosticCode })
      : { method: 'POST' },
  );
export const removeExtension = (id: string) =>
  requestNoContent(`/api/extensions/${encodeURIComponent(id)}`, {
    method: 'DELETE',
  });
