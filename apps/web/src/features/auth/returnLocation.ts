import { returnToStorageKey } from '../../app/storageKeys';
import {
  readStored,
  storageAreas,
  writeStored,
} from '../../components/browserStorage';

/** Remembers the current location to return to after signing in. */
export const rememberReturnLocation = () =>
  writeStored(
    storageAreas.session,
    returnToStorageKey,
    `${window.location.pathname}${window.location.search}${window.location.hash}`,
  );

/**
 * Leaves for the identity provider. The server checks this address too, but it
 * comes from a provider's metadata and a `javascript:` URL would run here.
 */
export const leaveForProvider = (authorizationUrl: string) => {
  const { protocol } = new URL(authorizationUrl);
  if (protocol !== 'https:' && protocol !== 'http:')
    throw new Error('The identity provider address is not a web address.');
  window.location.assign(authorizationUrl);
};

/**
 * The remembered location, forgotten once read. Only same-origin paths are
 * returned, so a stored value cannot redirect elsewhere.
 */
export const takeReturnLocation = () => {
  const returnTo = readStored(storageAreas.session, returnToStorageKey);
  writeStored(storageAreas.session, returnToStorageKey, null);
  return returnTo?.startsWith('/') && !returnTo.startsWith('//')
    ? returnTo
    : undefined;
};
