// @vitest-environment jsdom
import { act, renderHook } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { useRecentlyPreviewedEntities } from './useRecentlyPreviewedEntities';

describe('useRecentlyPreviewedEntities', () => {
  afterEach(() => {
    vi.useRealTimers();
    vi.unstubAllGlobals();
  });

  it('creates picker tokens when randomUUID is unavailable', () => {
    vi.stubGlobal('crypto', {
      getRandomValues: (values: Uint32Array) => {
        values.set([1, 2, 3, 4]);
        return values;
      },
    });

    const { result } = renderHook(() => useRecentlyPreviewedEntities());

    expect(result.current.previewHref('entity-id')).toContain(
      'relationshipPicker=1-2-3-4',
    );
  });

  it('accepts a same-origin selection message with its picker token', () => {
    const onSelect = vi.fn();
    const { result } = renderHook(() => useRecentlyPreviewedEntities(onSelect));
    const previewUrl = new URL(
      result.current.previewHref('entity-id'),
      window.location.origin,
    );
    const token = previewUrl.searchParams.get('relationshipPicker');

    act(() => {
      window.dispatchEvent(
        new MessageEvent('message', {
          data: {
            type: 'attricat.relationship-picker.select',
            token,
            entityId: 'entity-id',
          },
          origin: window.location.origin,
        }),
      );
    });

    expect(onSelect).toHaveBeenCalledWith('entity-id');
  });

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
