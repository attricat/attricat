// @vitest-environment jsdom
import { act, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { useState, type ReactElement } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import '../../i18n';
import type { GalleryFile } from './ImageGallery';
import { ImageGalleryEditor } from './ImageGalleryEditor';
import { getFileMetadata } from './api';

vi.mock('./api', () => ({
  getFileMetadata: vi.fn(),
  fileDownloadUrl: (id: string) => `/files/${id}`,
}));

const files: GalleryFile[] = [
  { id: '11111111-1111-4111-8111-111111111111', filename: 'front.png' },
  { id: '22222222-2222-4222-8222-222222222222', filename: 'back.png' },
  { id: '33333333-3333-4333-8333-333333333333', filename: 'side.png' },
];
beforeEach(() => {
  vi.mocked(getFileMetadata).mockRejectedValue(new Error('Unavailable'));
});

const ids = (order: readonly GalleryFile[]) => order.map((file) => file.id);

const renderEditor = (ui: ReactElement) => {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return render(ui, {
    wrapper: ({ children }) => (
      <QueryClientProvider client={client}>{children}</QueryClientProvider>
    ),
  });
};

const previewNames = () =>
  screen
    .getAllByRole('button', { name: /^Preview / })
    .map((button) => button.getAttribute('aria-label'));

/** Mirrors the file editor: saving disables the gallery until it settles. */
const SavingGallery = ({
  save,
}: {
  save: (fileIds: string[]) => Promise<boolean>;
}) => {
  const [shown, setShown] = useState(files);
  const [saving, setSaving] = useState(false);
  return (
    <ImageGalleryEditor
      files={shown}
      ordered
      disabled={saving}
      onChange={async (fileIds) => {
        setSaving(true);
        const saved = await save(fileIds);
        if (saved)
          setShown(fileIds.map((id) => files.find((file) => file.id === id)!));
        setSaving(false);
        return saved;
      }}
    />
  );
};

const deferred = () => {
  let resolve!: (value: boolean) => void;
  const promise = new Promise<boolean>((done) => {
    resolve = done;
  });
  return { promise, resolve };
};

describe('ImageGalleryEditor', () => {
  it('labels each drag handle and describes keyboard reordering', async () => {
    renderEditor(
      <ImageGalleryEditor
        files={files}
        ordered
        disabled={false}
        onChange={vi.fn()}
      />,
    );
    const handles = screen.getAllByRole('button', {
      name: /^Drag to reorder/,
    });
    expect(handles.map((handle) => handle.getAttribute('aria-label'))).toEqual(
      files.map((file) => `Drag to reorder ${file.filename}`),
    );
    await waitFor(() =>
      expect(
        document.getElementById(
          handles[0].getAttribute('aria-describedby') ?? '',
        )?.textContent,
      ).toMatch(/press space or enter/i),
    );
  });

  it('disables moving past either end of the gallery', () => {
    renderEditor(
      <ImageGalleryEditor
        files={files}
        ordered
        disabled={false}
        onChange={vi.fn()}
      />,
    );
    const button = (name: string) =>
      screen.getByRole<HTMLButtonElement>('button', { name });
    expect(button('Move front.png earlier').disabled).toBe(true);
    expect(button('Move front.png later').disabled).toBe(false);
    expect(button('Move side.png earlier').disabled).toBe(false);
    expect(button('Move side.png later').disabled).toBe(true);
  });

  it('shows the new order while saving and reverts it when the save fails', async () => {
    const user = userEvent.setup();
    const save = deferred();
    const onChange = vi.fn(() => save.promise);
    renderEditor(
      <ImageGalleryEditor
        files={files}
        ordered
        disabled={false}
        onChange={onChange}
      />,
    );
    await user.click(
      screen.getByRole('button', { name: 'Move back.png earlier' }),
    );
    expect(onChange).toHaveBeenCalledWith(ids([files[1], files[0], files[2]]));
    expect(previewNames()).toEqual([
      'Preview back.png',
      'Preview front.png',
      'Preview side.png',
    ]);
    await act(async () => save.resolve(false));
    expect(previewNames()).toEqual(files.map((f) => `Preview ${f.filename}`));
  });

  it('keeps keyboard focus on the move control after the save settles', async () => {
    const user = userEvent.setup();
    let save = deferred();
    renderEditor(<SavingGallery save={() => save.promise} />);
    const later = screen.getByRole('button', { name: 'Move back.png later' });
    later.focus();
    // Browsers drop focus from a control once saving disables it; jsdom does
    // not, so this checks where focus lands rather than that it returns.
    await user.keyboard('{Enter}');
    await act(async () => save.resolve(true));
    expect(previewNames()).toEqual([
      'Preview front.png',
      'Preview side.png',
      'Preview back.png',
    ]);
    // Now last, so its "later" arrow is disabled: focus the opposite arrow.
    expect(document.activeElement).toBe(
      screen.getByRole('button', { name: 'Move back.png earlier' }),
    );
    save = deferred();
    await user.keyboard('{Enter}');
    await act(async () => save.resolve(true));
    expect(document.activeElement).toBe(
      screen.getByRole('button', { name: 'Move back.png earlier' }),
    );
  });

  it('cancels removal without saving and confirms it with the remaining order', async () => {
    const user = userEvent.setup();
    const onChange = vi.fn().mockResolvedValue(true);
    renderEditor(
      <ImageGalleryEditor
        files={files}
        ordered={false}
        disabled={false}
        onChange={onChange}
      />,
    );
    // Unordered galleries offer no reordering controls.
    expect(screen.queryByRole('button', { name: /^(Drag|Move) / })).toBeNull();
    await user.click(screen.getByRole('button', { name: 'Remove back.png' }));
    await user.click(screen.getByRole('button', { name: 'Cancel' }));
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
    expect(onChange).not.toHaveBeenCalled();
    await user.click(screen.getByRole('button', { name: 'Remove back.png' }));
    await user.click(screen.getByRole('button', { name: 'Remove attachment' }));
    expect(onChange).toHaveBeenCalledWith(ids([files[0], files[2]]));
  });

  it('blocks every mutation while disabled, including an open removal', async () => {
    const user = userEvent.setup();
    const onChange = vi.fn().mockResolvedValue(true);
    const view = renderEditor(
      <ImageGalleryEditor
        files={files}
        ordered
        disabled={false}
        onChange={onChange}
      />,
    );
    await user.click(screen.getByRole('button', { name: 'Remove front.png' }));
    view.rerender(
      <ImageGalleryEditor files={files} ordered disabled onChange={onChange} />,
    );
    const confirm = screen.getByRole<HTMLButtonElement>('button', {
      name: 'Remove attachment',
    });
    expect(confirm.disabled).toBe(true);
    for (const button of screen.getAllByRole<HTMLButtonElement>('button', {
      name: /^(Drag|Move|Remove) .*\.png/,
      hidden: true,
    }))
      expect(button.disabled).toBe(true);
    expect(onChange).not.toHaveBeenCalled();
  });
});
