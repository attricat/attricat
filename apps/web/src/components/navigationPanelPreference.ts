// Remembers that the user collapsed the desktop navigation panel so it stays
// closed across reloads until they open a panel from the rail again.
const storageKey = 'attricat.navigation-panel-collapsed';

export const savedNavigationPanelCollapsed = () => {
  try {
    return localStorage.getItem(storageKey) === 'true';
  } catch {
    return false;
  }
};

export const saveNavigationPanelCollapsed = (collapsed: boolean) => {
  try {
    if (collapsed) localStorage.setItem(storageKey, 'true');
    else localStorage.removeItem(storageKey);
  } catch {
    // Collapsing still works for this page view when storage is unavailable.
  }
};
