// @vitest-environment jsdom
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { ImageGallery } from './ImageGallery';
import { ImageGalleryEditor } from './ImageGalleryEditor';
import { getFileMetadata } from './api';
import type { FileMetadata } from './schemas';
import { GALLERY_PAGE_SIZE } from './constants';
import type { ReactElement } from 'react';

vi.mock('./api', () => ({
  getFileMetadata: vi.fn(),
  fileDownloadUrl: (id: string, variant?: string) =>
    `/files/${id}/${variant ?? 'original'}`,
}));
const files = [
  { id: '11111111-1111-4111-8111-111111111111', filename: 'front.png' },
  { id: '22222222-2222-4222-8222-222222222222', filename: 'back.png' },
];
const renderGallery = (ui: ReactElement) => {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return render(ui, {
    wrapper: ({ children }) => (
      <QueryClientProvider client={client}>{children}</QueryClientProvider>
    ),
  });
};
beforeEach(() => {
  vi.stubGlobal(
    'ResizeObserver',
    class {
      observe() {}
      disconnect() {}
    },
  );
  vi.mocked(getFileMetadata)
    .mockReset()
    .mockImplementation(
      async (id) =>
        ({
          id,
          filename:
            files.find((file) => file.id === id)?.filename ?? 'photo.png',
          status: 'ready',
          variants: [
            { kind: 'thumbnail', width: 320, height: 200 },
            { kind: 'display', width: 1600, height: 1000 },
          ],
        }) as FileMetadata,
    );
});

describe('ImageGallery', () => {
  it('opens display variants, zooms, navigates, resets and restores focus on Escape', async () => {
    const user = userEvent.setup();
    renderGallery(<ImageGallery files={files} />);
    const trigger = screen.getByRole('button', { name: 'Preview front.png' });
    trigger.focus();
    await user.keyboard('{Enter}');
    const image = await screen.findByAltText('front.png');
    expect(image.getAttribute('src')).toBe(`/files/${files[0].id}/display`);
    fireEvent.load(image);
    await user.click(screen.getByRole('button', { name: 'Zoom in' }));
    expect(screen.getByText('150% of fitted size')).toBeTruthy();
    await user.click(screen.getByRole('button', { name: 'Fit image' }));
    expect(screen.getByText('100% of fitted size')).toBeTruthy();
    await user.click(screen.getByRole('button', { name: 'Zoom in' }));
    await user.click(screen.getByRole('button', { name: 'Next image' }));
    expect(await screen.findByAltText('back.png')).toBeTruthy();
    expect(screen.getByText('100% of fitted size')).toBeTruthy();
    expect(screen.getByText('2 of 2')).toBeTruthy();
    await user.keyboard('{Escape}');
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
    expect(document.activeElement).toBe(trigger);
  });

  it('bounds thumbnail requests with pagination and preserves file panels', async () => {
    const many = Array.from({ length: GALLERY_PAGE_SIZE + 1 }, (_, index) => ({
      id: `id-${index}`,
      filename: `photo-${index}.png`,
    }));
    renderGallery(
      <ImageGallery
        files={many}
        renderFilePanel={(id) => <span>Panel {id}</span>}
      />,
    );
    expect(screen.getAllByRole('button', { name: /^Preview/ })).toHaveLength(
      GALLERY_PAGE_SIZE,
    );
    expect(screen.queryByText(`Panel id-${GALLERY_PAGE_SIZE}`)).toBeNull();
    fireEvent.click(screen.getByRole('button', { name: 'Go to page 2' }));
    expect(screen.getAllByRole('button', { name: /^Preview/ })).toHaveLength(1);
    expect(screen.getByText(`Panel id-${GALLERY_PAGE_SIZE}`)).toBeTruthy();
  });

  it('renders an empty state and closes when the selected attachment disappears', async () => {
    const view = renderGallery(<ImageGallery files={files} />);
    fireEvent.click(screen.getByRole('button', { name: 'Preview front.png' }));
    expect(await screen.findByRole('dialog')).toBeTruthy();
    view.rerender(<ImageGallery files={[]} />);
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
    expect(screen.getByText('No images attached.')).toBeTruthy();
    view.rerender(<ImageGallery files={files} />);
    expect(screen.queryByRole('dialog')).toBeNull();
  });

  it('shows unavailable states for denied metadata without perpetual loading', async () => {
    vi.mocked(getFileMetadata).mockRejectedValue(new Error('Denied'));
    renderGallery(<ImageGallery files={[files[0]]} />);
    fireEvent.click(screen.getByRole('button', { name: 'Preview front.png' }));
    expect(await screen.findByText(/Image unavailable/)).toBeTruthy();
    expect(
      screen.queryByRole('progressbar', {
        name: 'Image is loading or processing',
      }),
    ).toBeNull();
    expect(
      screen.queryByRole('link', { name: 'Download front.png' }),
    ).toBeNull();
  });

  it('requires removal confirmation, preserves order, and disables all mutations', async () => {
    const onChange = vi.fn().mockResolvedValue(true);
    const view = renderGallery(
      <ImageGalleryEditor
        files={files}
        ordered
        disabled={false}
        onChange={onChange}
      />,
    );
    fireEvent.click(
      screen.getByRole('button', { name: 'Move front.png later' }),
    );
    expect(onChange).toHaveBeenLastCalledWith([files[1].id, files[0].id]);
    fireEvent.click(screen.getByRole('button', { name: 'Remove front.png' }));
    expect(onChange).toHaveBeenCalledTimes(1);
    fireEvent.click(screen.getByRole('button', { name: 'Remove attachment' }));
    await waitFor(() =>
      expect(onChange).toHaveBeenLastCalledWith([files[1].id]),
    );
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
    view.rerender(
      <ImageGalleryEditor files={files} ordered disabled onChange={onChange} />,
    );
    for (const button of screen.getAllByRole('button', {
      name: /^(Drag|Move|Remove)/,
    }))
      expect((button as HTMLButtonElement).disabled).toBe(true);
    expect(
      (
        screen.getByRole('button', {
          name: 'Preview front.png',
        }) as HTMLButtonElement
      ).disabled,
    ).toBe(false);
  });
});
