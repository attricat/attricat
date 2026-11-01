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
import { extensionsApiPath, extensionPath } from './apiPaths';

export type { ExtensionDetail, ExtensionInstallation } from './schemas';

export const discoverExtensions = () =>
  request('/api/extension-registries/discover', z.array(discoveredSchema));
export const registryDetails = (owner: string, repository: string) =>
  request(
    `/api/extension-registries/extensions/${encodeURIComponent(owner)}/${encodeURIComponent(repository)}`,
    registryDetailsSchema,
  );
export const installedExtensions = () =>
  request(extensionsApiPath, z.array(installationSchema));
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
  request(extensionPath(id), detailSchema);
export const installExtension = (input: ExtensionReleaseReference) =>
  request(extensionsApiPath, installationSchema, jsonRequest('POST', input));
export const sideloadExtension = (archive: File) =>
  request(`${extensionsApiPath}/sideload`, installationSchema, {
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
    `${extensionPath(id)}/upgrade`,
    installationSchema,
    jsonRequest('POST', input),
  );
export const upgradeExtensionFromArchive = (id: string, archive: File) =>
  request(`${extensionPath(id)}/upgrade-archive`, installationSchema, {
    method: 'POST',
    headers: { 'Content-Type': 'application/zstd' },
    body: archive,
  });
export const configureExtension = (id: string, configuration: unknown) =>
  request(
    `${extensionPath(id)}/configure`,
    installationSchema,
    jsonRequest('PUT', { configuration }),
  );
export const grantExtension = (
  id: string,
  grant_kind: GrantKind,
  grant_id: string,
) =>
  requestNoContent(
    `${extensionPath(id)}/grants`,
    jsonRequest('POST', { grant_kind, grant_id }),
  );
export const revokeExtensionGrant = (
  id: string,
  kind: GrantKind,
  grant: string,
) =>
  requestNoContent(
    `${extensionPath(id)}/grants/${encodeURIComponent(kind)}/${encodeURIComponent(grant)}`,
    { method: 'DELETE' },
  );
export const lifecycleExtension = (
  id: string,
  action: ExtensionLifecycleAction,
) =>
  request(
    `${extensionPath(id)}/${action}`,
    installationSchema,
    action === 'quarantine'
      ? jsonRequest('POST', { diagnostic_code: manualQuarantineDiagnosticCode })
      : { method: 'POST' },
  );
export const removeExtension = (id: string) =>
  requestNoContent(extensionPath(id), {
    method: 'DELETE',
  });
