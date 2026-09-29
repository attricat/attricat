import { lastBlueprintStorageKey } from './constants';

export const getLastBlueprint = () => {
  try {
    return sessionStorage.getItem(lastBlueprintStorageKey) || undefined;
  } catch {
    return undefined;
  }
};

export const setLastBlueprint = (blueprint: string) => {
  try {
    sessionStorage.setItem(lastBlueprintStorageKey, blueprint);
  } catch {
    // Storage can be unavailable in hardened browser contexts.
  }
};
