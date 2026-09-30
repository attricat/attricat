import { INSPECTOR_ENABLED_STORAGE_KEY } from './constants';

const storedFlagEnabled = () => {
  try {
    return localStorage.getItem(INSPECTOR_ENABLED_STORAGE_KEY) === 'true';
  } catch {
    return false;
  }
};

/**
 * Development servers started with `CATALOG_DEVTOOLS=true` always show the
 * Inspector. Any other build, including production, shows it only when this
 * browser opted in through local storage. Read once at startup, so toggling
 * the flag takes effect on the next page load.
 */
export const inspectorEnabled =
  (import.meta.env.DEV && __CATALOG_DEVTOOLS__) || storedFlagEnabled();
