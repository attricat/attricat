// @vitest-environment jsdom
import { act, renderHook } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  scrollRelationshipPickerToTop,
  useRecentlyPreviewedRecords,
  relationshipPickerManyMessageType,
  relationshipPickerMessageType,
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
      getRandomValues: (values: Uint8Array) => values.fill(0xff),
    });

    const { result } = renderHook(() => useRecentlyPreviewedRecords());

    expect(result.current.previewHref('record-id')).toContain(
      'relationshipPicker=ffffffff-ffff-4fff-bfff-ffffffffffff',
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
            type: relationshipPickerMessageType,
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

  it('accepts records chosen in an Explorer tab, checking each one', () => {
    const onSelect = vi.fn();
    const onSelectMany = vi.fn();
    const { result } = renderHook(() =>
      useRecentlyPreviewedRecords(onSelect, onSelectMany),
    );
    const token = result.current.pickerToken;
    const first = '8d6f1c2a-1b2c-4d3e-8f4a-5b6c7d8e9f01';
    const second = '8d6f1c2a-1b2c-4d3e-8f4a-5b6c7d8e9f02';
    const send = (data: unknown, origin = window.location.origin) =>
      act(() => {
        window.dispatchEvent(new MessageEvent('message', { data, origin }));
      });
    const many = (extra: Record<string, unknown>) => ({
      type: relationshipPickerManyMessageType,
      token,
      recordIds: [first, second],
      labels: { [first]: 'Bolt', [second]: 7 },
      ...extra,
    });

    send(many({ token: 'another-picker' }));
    send(many({}), 'https://elsewhere.example');
    send(many({ recordIds: ['not-a-record'] }));
    send(many({ recordIds: Array.from({ length: 51 }, () => first) }));
    expect(onSelectMany).not.toHaveBeenCalled();

    send(many({}));
    // Labels that are not text are left out.
    expect(onSelectMany).toHaveBeenCalledWith([first, second], {
      [first]: 'Bolt',
    });
    expect(onSelect).not.toHaveBeenCalled();
  });
});
