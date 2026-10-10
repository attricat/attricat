// @vitest-environment jsdom
import { act, renderHook } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  scrollRelationshipPickerToTop,
  useRecentlyPreviewedRecords,
} from './useRecentlyPreviewedRecords';

describe('useRecentlyPreviewedRecords', () => {
  afterEach(() => {
    vi.useRealTimers();
    vi.unstubAllGlobals();
  });

  it('scrolls the containing picker dialog to the top', () => {
    const content = document.createElement('div');
    content.className = 'MuiDialogContent-root';
    const child = document.createElement('div');
    content.append(child);
    const scrollTo = vi.fn();
    content.scrollTo = scrollTo;

    scrollRelationshipPickerToTop(child);

    expect(scrollTo).toHaveBeenCalledWith({ top: 0 });
  });

  it('creates picker tokens when randomUUID is unavailable', () => {
    vi.stubGlobal('crypto', {
      getRandomValues: (values: Uint32Array) => {
        values.set([1, 2, 3, 4]);
        return values;
      },
    });

    const { result } = renderHook(() => useRecentlyPreviewedRecords());

    expect(result.current.previewHref('record-id')).toContain(
      'relationshipPicker=1-2-3-4',
    );
  });

  it('accepts a same-origin selection message with its picker token', () => {
    const onSelect = vi.fn();
    const { result } = renderHook(() => useRecentlyPreviewedRecords(onSelect));
    const previewUrl = new URL(
      result.current.previewHref('record-id'),
      window.location.origin,
    );
    const token = previewUrl.searchParams.get('relationshipPicker');

    act(() => {
      window.dispatchEvent(
        new MessageEvent('message', {
          data: {
            type: 'attricat.relationship-picker.select',
            token,
            recordId: 'record-id',
          },
          origin: window.location.origin,
        }),
      );
    });

    expect(onSelect).toHaveBeenCalledWith('record-id');
  });

  it('clears preview highlights five seconds after returning to the picker', () => {
    vi.useFakeTimers();
    const { result } = renderHook(() => useRecentlyPreviewedRecords());

    act(() => result.current.markPreviewed('record-id'));
    expect(result.current.isPreviewed('record-id')).toBe(true);

    act(() => {
      window.dispatchEvent(new Event('blur'));
      window.dispatchEvent(new Event('focus'));
      vi.advanceTimersByTime(4_999);
    });
    expect(result.current.isPreviewed('record-id')).toBe(true);

    act(() => vi.advanceTimersByTime(1));
    expect(result.current.isPreviewed('record-id')).toBe(false);
  });
});
