import { readStored, writeStored, storageAreas } from './browserStorage';

// Remembers that the user collapsed the desktop navigation panel so it stays
// closed across reloads until they open a panel from the rail again.
const storageKey = 'attricat.navigation-panel-collapsed';
const collapsedValue = 'true';

export const savedNavigationPanelCollapsed = () =>
  readStored(storageAreas.local, storageKey) === collapsedValue;

export const saveNavigationPanelCollapsed = (collapsed: boolean) =>
  writeStored(
    storageAreas.local,
    storageKey,
    collapsed ? collapsedValue : null,
  );
