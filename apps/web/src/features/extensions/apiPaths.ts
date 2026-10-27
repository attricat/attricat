/** The extension API. */
export const extensionsApiPath = '/api/extensions';

/** The API path of one installed extension. */
export const extensionPath = (extensionId: string) =>
  `${extensionsApiPath}/${encodeURIComponent(extensionId)}`;

/** The API path of one client contribution of an extension. */
export const contributionPath = (extensionId: string, contributionId: string) =>
  `${extensionPath(extensionId)}/${encodeURIComponent(contributionId)}`;
