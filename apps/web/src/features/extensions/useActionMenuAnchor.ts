import { useState, type MouseEvent } from 'react';
import { useActionDialogStore } from './actionDialogStore';

/**
 * Where a menu with extension actions is open. An action that opens the host
 * dialog hands off to it, so the menu closes; the dialog keeps its captured
 * selection after the menu and its frames are gone.
 */
export const useActionMenuAnchor = () => {
  const [anchor, setAnchor] = useState<HTMLElement | null>(null);
  const dialogKey = useActionDialogStore((state) => state.dialog?.key);
  const [openedDialogKey, setOpenedDialogKey] = useState(dialogKey);
  if (anchor && dialogKey !== undefined && dialogKey !== openedDialogKey) {
    setOpenedDialogKey(dialogKey);
    setAnchor(null);
  }
  return {
    anchor,
    close: () => setAnchor(null),
    open: (event: MouseEvent<HTMLElement>) => {
      setOpenedDialogKey(dialogKey);
      setAnchor(event.currentTarget);
    },
  };
};
