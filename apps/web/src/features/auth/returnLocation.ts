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
