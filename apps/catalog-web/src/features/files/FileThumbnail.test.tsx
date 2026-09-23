// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { act, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { fileDownloadUrl, getFileMetadata } from './api';
import { FileThumbnail, ThumbnailPreview } from './FileThumbnail';

vi.mock('./api', () => ({
  fileDownloadUrl: vi.fn(),
  getFileMetadata: vi.fn(),
}));

afterEach(() => {
  vi.useRealTimers();
});

const retry = () => {
  act(() => {
    vi.advanceTimersByTime(1_000);
  });
};

describe('ThumbnailPreview', () => {
  it('does not download the original file when no thumbnail variant exists', async () => {
    vi.mocked(getFileMetadata).mockResolvedValue({
      id: 'file-id',
      filename: 'document.pdf',
      mime_type: 'application/pdf',
      byte_size: 128,
      sha256: 'a'.repeat(64),
      status: 'ready',
      variants: [],
    });
    const client = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    });
    const { container } = render(
      <QueryClientProvider client={client}>
        <FileThumbnail
          file={{ id: 'file-id', filename: 'document.pdf' }}
          size={48}
        />
      </QueryClientProvider>,
    );
    expect(await screen.findByText('Thumbnail unavailable')).toBeTruthy();
    expect(container.querySelector('img')).toBeNull();
    expect(fileDownloadUrl).not.toHaveBeenCalled();
  });

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

  it('bounds remembered thumbnail sources across virtualized rows', () => {
    const view = render(
      <ThumbnailPreview
        filename="first.png"
        size={48}
        source="/cache-first"
        unavailable={false}
      />,
    );
    fireEvent.load(view.container.querySelector('img')!);
    for (let index = 0; index < 256; index += 1) {
      view.rerender(
        <ThumbnailPreview
          filename="next.png"
          size={48}
          source={`/cache-${index}`}
          unavailable={false}
        />,
      );
      fireEvent.load(view.container.querySelector('img')!);
    }
    view.rerender(
      <ThumbnailPreview
        filename="first.png"
        size={48}
        source="/cache-first"
        unavailable={false}
      />,
    );
    expect(
      screen
        .getByRole('img', { name: 'Thumbnail for first.png' })
        .getAttribute('aria-busy'),
    ).toBe('true');
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
