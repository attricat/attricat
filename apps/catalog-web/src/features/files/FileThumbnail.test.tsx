// @vitest-environment jsdom
import { act, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { ThumbnailPreview } from './FileThumbnail';

afterEach(() => {
  vi.useRealTimers();
});

const retry = () => {
  act(() => {
    vi.advanceTimersByTime(1_000);
  });
};

describe('ThumbnailPreview', () => {
  it('stops retrying a broken thumbnail and shows an unavailable state', () => {
    vi.useFakeTimers();
    const { container } = render(
      <ThumbnailPreview
        filename="shirt.png"
        size={48}
        source="/thumbnail"
        unavailable={false}
      />,
    );

    for (let attempt = 0; attempt < 3; attempt += 1) {
      fireEvent.error(container.querySelector('img')!);
      retry();
    }

    fireEvent.error(container.querySelector('img')!);

    expect(screen.getByText('Thumbnail unavailable')).toBeTruthy();
    expect(vi.getTimerCount()).toBe(0);
  });

  it('cancels a pending retry when the thumbnail source changes or unmounts', () => {
    vi.useFakeTimers();
    const clearTimeout = vi.spyOn(window, 'clearTimeout');
    const { container, rerender, unmount } = render(
      <ThumbnailPreview
        filename="shirt.png"
        size={48}
        source="/old-thumbnail"
        unavailable={false}
      />,
    );

    fireEvent.error(container.querySelector('img')!);
    rerender(
      <ThumbnailPreview
        filename="shirt.png"
        size={48}
        source="/new-thumbnail"
        unavailable={false}
      />,
    );

    expect(clearTimeout).toHaveBeenCalled();
    retry();
    expect(container.querySelector('img')?.getAttribute('src')).toContain(
      '/new-thumbnail',
    );
    expect(container.querySelector('img')?.getAttribute('src')).not.toContain(
      'retry=',
    );

    fireEvent.error(container.querySelector('img')!);
    unmount();
    expect(clearTimeout).toHaveBeenCalledTimes(2);
  });
});
