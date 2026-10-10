// @vitest-environment jsdom
import { act, renderHook } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { uploadFiles } from './api';
import { useQueuedFileUploads } from './queuedFileUploads';
import type { PendingFile } from './usePendingFileUploads';

vi.mock('./api', () => ({ uploadFiles: vi.fn() }));

const entityId = '123e4567-e89b-12d3-a456-426614174000';
const queued = (name: string): PendingFile => ({
  file: new File([name], name, { type: 'image/png' }),
  id: name,
  progress: 0,
});
const uploadResult = { files: [] } as unknown as Awaited<
  ReturnType<typeof uploadFiles>
>;

beforeEach(() => {
  vi.mocked(uploadFiles).mockReset();
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

  it('uploads files one at a time in queue order and reports every failure', async () => {
    const order: string[] = [];
    vi.mocked(uploadFiles).mockImplementation(async ({ files }) => {
      const [file] = files;
      order.push(file.name);
      if (file.name === 'b.png') throw new Error('Too large');
      if (file.name === 'm.pdf') throw 'offline';
      return uploadResult;
    });
    const { result } = renderHook(() => useQueuedFileUploads('product:1'));
    const files = {
      photos: [queued('a.png'), queued('b.png'), queued('c.png')],
      manual: [queued('m.pdf')],
    };
    let failed: Awaited<ReturnType<typeof result.current.uploadQueued>> = [];
    await act(async () => {
      failed = await result.current.uploadQueued(files, entityId, 'ctx');
    });
    // A failure does not stop the remaining files.
    expect(order).toEqual(['a.png', 'b.png', 'c.png', 'm.pdf']);
    expect(failed).toEqual([
      { filename: 'b.png', message: 'Too large' },
      { filename: 'm.pdf', message: undefined },
    ]);
    expect(uploadFiles).toHaveBeenCalledWith(
      expect.objectContaining({
        attributeCode: 'manual',
        contextId: 'ctx',
        entityId,
      }),
    );
  });

  it('reports a started upload as in progress even at 0%', async () => {
    let finish!: () => void;
    let report!: (progress: number) => void;
    vi.mocked(uploadFiles).mockImplementation(
      ({ onProgress }) =>
        new Promise((resolve) => {
          report = onProgress!;
          finish = () => resolve(uploadResult);
        }),
    );
    const { result } = renderHook(() => useQueuedFileUploads('product:1'));
    act(() => result.current.queue.update('photos', () => [queued('a.png')]));
    let done!: Promise<unknown>;
    act(() => {
      done = result.current.uploadQueued(
        result.current.queue.pending,
        entityId,
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
