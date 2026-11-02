import { useLayoutEffect, useRef, type RefObject } from 'react';

/**
 * Moves an input's cursor once its new value has rendered, which would
 * otherwise send the cursor to the end. Returns the function to ask for that.
 */
export const useCursorPlacement = (
  inputRef: RefObject<HTMLInputElement | HTMLTextAreaElement | null>,
) => {
  const pending = useRef<number | null>(null);
  useLayoutEffect(() => {
    if (pending.current === null || !inputRef.current) return;
    inputRef.current.setSelectionRange(pending.current, pending.current);
    pending.current = null;
  });
  return (cursor: number) => {
    pending.current = cursor;
  };
};
