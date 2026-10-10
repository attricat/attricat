// @vitest-environment jsdom
import { act, renderHook } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { uploadStagedFiles } from './api';
import { useQueuedFileUploads } from './queuedFileUploads';
import type { PendingFile } from './usePendingFileUploads';

vi.mock('./api', () => ({ uploadStagedFiles: vi.fn() }));

const blueprintId = '123e4567-e89b-12d3-a456-426614174000';
const queued = (name: string): PendingFile => ({
  file: new File([name], name, { type: 'image/png' }),
  id: name,
  progress: 0,
});
const stagedResult = (id: string) =>
  ({ files: [{ id }] }) as unknown as Awaited<
    ReturnType<typeof uploadStagedFiles>
  >;

beforeEach(() => {
  vi.mocked(uploadStagedFiles).mockReset();
});

describe('useQueuedFileUploads', () => {
  it('keeps each attribute queue separate', () => {
    const { result } = renderHook(() => useQueuedFileUploads('product:1'));
    act(() => {
      result.current.queue.update('photos', (items) => [
        ...items,
        queued('a.png'),
      ]);
      result.current.queue.update('manual', () => [queued('m.pdf')]);
      result.current.queue.update('photos', (items) => [
        ...items,
        queued('b.png'),
      ]);
    });
    expect(result.current.queue.pending).toEqual({
      photos: [queued('a.png'), queued('b.png')],
      manual: [queued('m.pdf')],
    });
  });

  it('discards the queue when the scope changes', () => {
    const { rerender, result } = renderHook(
      ({ scope }) => useQueuedFileUploads(scope),
      { initialProps: { scope: 'product:1' as string | undefined } },
    );
    act(() => result.current.queue.update('photos', () => [queued('a.png')]));
    rerender({ scope: 'product:2' });
    expect(result.current.queue.pending).toEqual({});
    act(() => result.current.queue.update('photos', () => [queued('b.png')]));
    // Returning to the earlier scope does not bring back its old files.
    rerender({ scope: 'product:1' });
    expect(result.current.queue.pending).toEqual({});
  });

  it('stages files one at a time in queue order and reports every failure', async () => {
    const order: string[] = [];
    vi.mocked(uploadStagedFiles).mockImplementation(async ({ files }) => {
      const [file] = files;
      order.push(file.name);
      if (file.name === 'b.png') throw new Error('Too large');
      if (file.name === 'm.pdf') throw 'offline';
      return stagedResult(`staged-${file.name}`);
    });
    const { result } = renderHook(() => useQueuedFileUploads('product:1'));
    const files = {
      photos: [queued('a.png'), queued('b.png'), queued('c.png')],
      manual: [queued('m.pdf')],
    };
    act(() => {
      result.current.queue.update('photos', () => files.photos);
      result.current.queue.update('manual', () => files.manual);
    });
    let staged!: Awaited<ReturnType<typeof result.current.stageQueued>>;
    await act(async () => {
      staged = await result.current.stageQueued(files, blueprintId, 'ctx');
    });
    // A failure does not stop the remaining files.
    expect(order).toEqual(['a.png', 'b.png', 'c.png', 'm.pdf']);
    expect(staged.failed).toEqual([
      { filename: 'b.png', message: 'Too large' },
      { filename: 'm.pdf', message: undefined },
    ]);
    expect(staged.staged).toEqual([
      {
        attribute_code: 'photos',
        context_id: 'ctx',
        file_ids: ['staged-a.png', 'staged-c.png'],
      },
    ]);
    expect(uploadStagedFiles).toHaveBeenCalledWith(
      expect.objectContaining({
        attributeCode: 'manual',
        blueprintId,
        contextId: 'ctx',
      }),
    );
    // Failed files stay queued with their error; staged ones stay removable.
    const photos = result.current.queue.pending.photos;
    expect(photos.map((item) => [item.stagedFileId, item.progress])).toEqual([
      ['staged-a.png', 0],
      [undefined, 0],
      ['staged-c.png', 0],
    ]);
    expect(photos[1].error).toBe('Too large');
    expect(result.current.queue.pending.manual[0].error).toBeTruthy();
  });

  it('does not upload a staged file again, unless its staging is forgotten', async () => {
    vi.mocked(uploadStagedFiles).mockImplementation(async ({ files }) =>
      stagedResult(`staged-${files[0].name}`),
    );
    const { result } = renderHook(() => useQueuedFileUploads('product:1'));
    act(() => result.current.queue.update('photos', () => [queued('a.png')]));
    await act(async () => {
      await result.current.stageQueued(
        result.current.queue.pending,
        blueprintId,
        null,
      );
    });
    let again!: Awaited<ReturnType<typeof result.current.stageQueued>>;
    await act(async () => {
      again = await result.current.stageQueued(
        result.current.queue.pending,
        blueprintId,
        null,
      );
    });
    expect(uploadStagedFiles).toHaveBeenCalledTimes(1);
    expect(again.staged[0].file_ids).toEqual(['staged-a.png']);
    act(() => result.current.forgetStaged());
    expect(result.current.queue.pending.photos[0].stagedFileId).toBeUndefined();
    await act(async () => {
      await result.current.stageQueued(
        result.current.queue.pending,
        blueprintId,
        null,
      );
    });
    expect(uploadStagedFiles).toHaveBeenCalledTimes(2);
  });

  it('reports a started upload as in progress even at 0%', async () => {
    let finish!: () => void;
    let report!: (progress: number) => void;
    vi.mocked(uploadStagedFiles).mockImplementation(
      ({ onProgress }) =>
        new Promise((resolve) => {
          report = onProgress!;
          finish = () => resolve(stagedResult('staged'));
        }),
    );
    const { result } = renderHook(() => useQueuedFileUploads('product:1'));
    act(() => result.current.queue.update('photos', () => [queued('a.png')]));
    let done!: Promise<unknown>;
    act(() => {
      done = result.current.stageQueued(
        result.current.queue.pending,
        blueprintId,
        null,
      );
    });
    const progress = () => result.current.queue.pending.photos[0].progress;
    expect(progress()).toBe(1);
    act(() => report(0));
    expect(progress()).toBe(1);
    act(() => report(60));
    expect(progress()).toBe(60);
    await act(async () => {
      finish();
      await done;
    });
  });
});
