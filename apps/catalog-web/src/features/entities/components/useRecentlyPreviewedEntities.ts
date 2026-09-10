import { useCallback, useEffect, useRef, useState } from 'react';

const previewHighlightDuration = 5_000;

export const useRecentlyPreviewedEntities = () => {
  const [previewedIds, setPreviewedIds] = useState<Set<string>>(new Set());
  const leftPicker = useRef(false);
  const resetTimer = useRef<ReturnType<typeof setTimeout> | undefined>(
    undefined,
  );

  useEffect(() => {
    const handleBlur = () => {
      leftPicker.current = true;
    };
    const handleFocus = () => {
      if (!leftPicker.current) return;
      leftPicker.current = false;
      clearTimeout(resetTimer.current);
      resetTimer.current = setTimeout(
        () => setPreviewedIds(new Set()),
        previewHighlightDuration,
      );
    };
    window.addEventListener('blur', handleBlur);
    window.addEventListener('focus', handleFocus);
    return () => {
      window.removeEventListener('blur', handleBlur);
      window.removeEventListener('focus', handleFocus);
      clearTimeout(resetTimer.current);
    };
  }, []);

  const markPreviewed = useCallback((id: string) => {
    setPreviewedIds((current) => new Set(current).add(id));
  }, []);

  return {
    isPreviewed: (id: string) => previewedIds.has(id),
    markPreviewed,
  };
};
