import { useEffect } from 'react';

/** Ask the browser to confirm leaving the page while `enabled` is true. */
export const useBeforeUnloadWarning = (enabled: boolean) => {
  useEffect(() => {
    if (!enabled) return;
    const warnBeforeUnload = (event: BeforeUnloadEvent) => {
      event.preventDefault();
      event.returnValue = '';
    };
    window.addEventListener('beforeunload', warnBeforeUnload);
    return () => window.removeEventListener('beforeunload', warnBeforeUnload);
  }, [enabled]);
};
