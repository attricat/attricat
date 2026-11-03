import type { QueryClient } from '@tanstack/react-query';
import { z } from 'zod';
import {
  githubRepositoryPrefix,
  installationStates,
  lifecycleOperations,
  releaseTagSeparator,
  type GrantKind,
  type InstallationState,
  type LifecycleOperation,
} from './constants';
import { extensionQueryKeys } from './queryKeys';
import { extensionManagementQueryKeys } from './managementQueryKeys';
import { extensionExportQueryKeys } from '../extension-exports/queryKeys';

export const repositoryParts = (repository: string) =>
  repository.replace(githubRepositoryPrefix, '').split('/', 2);

/** Removes the `@release` suffix from an installation source. */
export const sourceRepository = (source: string) =>
  source.split(releaseTagSeparator, 1)[0];

/** Returns the release tag that follows the last `@` in an installation source. */
export const sourceReleaseTag = (source: string) =>
  source.split(releaseTagSeparator).at(-1);

const manifestNameSchema = z.looseObject({ name: z.string().trim().min(1) });
const manifestDescriptionSchema = z.looseObject({
  description: z.string().trim().min(1),
});

/** An installed extension's name from its manifest, or its ID without one. */
export const installedExtensionName = (installation: {
  extension_id: string;
  manifest: unknown;
}) => {
  const parsed = manifestNameSchema.safeParse(installation.manifest);
  return parsed.success ? parsed.data.name : installation.extension_id;
};

/** What an installed extension's manifest says it does, if anything. */
export const installedExtensionDescription = (installation: {
  manifest: unknown;
}) =>
  manifestDescriptionSchema.safeParse(installation.manifest).data?.description;

export const installationStateLabelKey = (state: InstallationState) =>
  `extensions.installationStates.${state}`;

export const grantKindLabelKey = (kind: GrantKind) =>
  `extensions.grantKinds.${kind}`;

const isLifecycleOperation = (
  operation: string,
): operation is LifecycleOperation =>
  (lifecycleOperations as readonly string[]).includes(operation);

/** Known lifecycle operations are translated; unknown server values are shown verbatim. */
export const lifecycleOperationLabel = (
  operation: string,
  translate: (key: string) => string,
) =>
  isLifecycleOperation(operation)
    ? translate(`extensions.lifecycleOperations.${operation}`)
    : operation;

const isInstallationState = (state: string): state is InstallationState =>
  (Object.values(installationStates) as string[]).includes(state);

/** Known installation states are translated; unknown server values are shown verbatim. */
export const lifecycleStateLabel = (
  state: string,
  translate: (key: string) => string,
) =>
  isInstallationState(state)
    ? translate(installationStateLabelKey(state))
    : state;

/** Refreshes every installation, layout, contribution and export query. */
export const invalidateExtensions = (client: QueryClient) => {
  void client.invalidateQueries({ queryKey: extensionManagementQueryKeys.all });
  void client.invalidateQueries({ queryKey: extensionQueryKeys.runtimeRoot() });
  void client.invalidateQueries({ queryKey: extensionExportQueryKeys.all });
};

const releaseVersionPattern =
  /^v?(\d+)\.(\d+)\.(\d+)(?:-([0-9A-Za-z.-]+))?(?:\+[0-9A-Za-z.-]+)?$/;

/**
 * Whether upgrading from `installed` to a release tag can succeed: the server
 * only accepts a greater version. A tag that is not a version is left for the
 * server to judge from the release's manifest.
 */
export const isNewerRelease = (tag: string, installed: string) => {
  const candidate = releaseVersionPattern.exec(tag);
  const current = releaseVersionPattern.exec(installed);
  if (!candidate || !current) return true;
  for (const part of [1, 2, 3]) {
    const difference = Number(candidate[part]) - Number(current[part]);
    if (difference !== 0) return difference > 0;
  }
  const [candidatePre, currentPre] = [candidate[4], current[4]];
  // A prerelease comes before its release.
  if (!candidatePre || !currentPre) return !candidatePre && Boolean(currentPre);
  return (
    candidatePre.localeCompare(currentPre, undefined, { numeric: true }) > 0
  );
};
