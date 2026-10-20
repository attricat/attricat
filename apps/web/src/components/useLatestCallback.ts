import { useCallback, useLayoutEffect, useRef } from 'react';

/**
 * A function with a stable identity that calls the latest `callback`, so
 * memoized children receiving it do not re-render when it is recreated. Call
 * it from events and effects, never while rendering.
 */
export const useLatestCallback = <Args extends unknown[], Result>(
  callback: (...args: Args) => Result,
) => {
  const latest = useRef(callback);
  useLayoutEffect(() => {
    latest.current = callback;
  });
  return useCallback((...args: Args) => latest.current(...args), []);
};
