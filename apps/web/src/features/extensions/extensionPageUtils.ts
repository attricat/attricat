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

export const repositoryParts = (repository: string) =>
  repository.replace(githubRepositoryPrefix, '').split('/', 2);

/** Removes the `@release` suffix from an installation source. */
export const sourceRepository = (source: string) =>
  source.split(releaseTagSeparator, 1)[0];

/** Returns the release tag that follows the last `@` in an installation source. */
export const sourceReleaseTag = (source: string) =>
  source.split(releaseTagSeparator).at(-1);

const manifestNameSchema = z.looseObject({ name: z.string().trim().min(1) });

/** An installed extension's name from its manifest, or its ID without one. */
export const installedExtensionName = (installation: {
  extension_id: string;
  manifest: unknown;
}) => {
  const parsed = manifestNameSchema.safeParse(installation.manifest);
  return parsed.success ? parsed.data.name : installation.extension_id;
};

export const installationStateColor = (state: InstallationState) =>
  state === installationStates.enabled
    ? 'success'
    : state === installationStates.quarantined
      ? 'error'
      : 'default';

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

/** Refreshes every installation, layout and contribution query. */
export const invalidateExtensions = (client: QueryClient) => {
  void client.invalidateQueries({ queryKey: extensionManagementQueryKeys.all });
  void client.invalidateQueries({ queryKey: extensionQueryKeys.runtimeRoot() });
};
