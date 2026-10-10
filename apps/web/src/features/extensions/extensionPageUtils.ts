import type { QueryClient } from '@tanstack/react-query';
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

export const installationStateColor = (state: InstallationState) =>
  state === 'enabled'
    ? 'success'
    : state === 'quarantined'
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
  (installationStates as readonly string[]).includes(state);

/** Known installation states are translated; unknown server values are shown verbatim. */
export const lifecycleStateLabel = (
  state: string,
  translate: (key: string) => string,
) =>
  isInstallationState(state)
    ? translate(installationStateLabelKey(state))
    : state;

export const invalidateExtensions = (
  client: QueryClient,
  extensionId?: string,
) => {
  void client.invalidateQueries({ queryKey: extensionManagementQueryKeys.all });
  void client.invalidateQueries({ queryKey: extensionQueryKeys.runtimeRoot() });
  if (extensionId)
    void client.invalidateQueries({
      queryKey: extensionManagementQueryKeys.detail(extensionId),
    });
};
