// @vitest-environment jsdom
import { act, renderHook } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { useRecentlyPreviewedEntities } from './useRecentlyPreviewedEntities';

describe('useRecentlyPreviewedEntities', () => {
  afterEach(() => vi.useRealTimers());

  it('clears preview highlights five seconds after returning to the picker', () => {
    vi.useFakeTimers();
    const { result } = renderHook(() => useRecentlyPreviewedEntities());

    act(() => result.current.markPreviewed('entity-id'));
    expect(result.current.isPreviewed('entity-id')).toBe(true);

    act(() => {
      window.dispatchEvent(new Event('blur'));
      window.dispatchEvent(new Event('focus'));
      vi.advanceTimersByTime(4_999);
    });
    expect(result.current.isPreviewed('entity-id')).toBe(true);

    act(() => vi.advanceTimersByTime(1));
    expect(result.current.isPreviewed('entity-id')).toBe(false);
  });
});
