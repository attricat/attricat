import {
  readStored,
  writeStored,
  storageAreas,
} from '../../components/browserStorage';
import { lastBlueprintStorageKey } from './constants';

export const getLastBlueprint = () =>
  readStored(storageAreas.session, lastBlueprintStorageKey) || undefined;

export const setLastBlueprint = (blueprint: string) =>
  writeStored(storageAreas.session, lastBlueprintStorageKey, blueprint);

export const clearLastBlueprint = () =>
  writeStored(storageAreas.session, lastBlueprintStorageKey, null);
