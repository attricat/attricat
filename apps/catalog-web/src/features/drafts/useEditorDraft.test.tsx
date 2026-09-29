// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { act, renderHook } from '@testing-library/react';
import type { ReactNode } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { authQueryKeys } from '../auth/queryKeys';
import { draftEditors, draftWriteDelayMs } from './constants';
import { draftStorageKey, writeDraft } from './draftStorage';
import { definitionDraftSchema } from './schemas';
import { useEditorDraft } from './useEditorDraft';

const workspaceId = 'workspace-1';
const userId = 'user-1';
const editor = draftEditors.blueprintRevision;
const storageKey = (resource: (string | number)[], user = userId) =>
  draftStorageKey({ editor, resource, userId: user, workspaceId });

type Props = {
  dirty: boolean;
  ready?: boolean;
  resource?: (string | number)[];
  source?: string | null;
  value: string;
};

const renderDraft = (initialProps: Props, user = userId) => {
  const client = new QueryClient({
    defaultOptions: { queries: { staleTime: Infinity } },
  });
  client.setQueryData(authQueryKeys.session(), {
    user_id: user,
    workspace_id: workspaceId,
  });
  const wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={client}>{children}</QueryClientProvider>
  );
  return renderHook(
    ({
      ready = true,
      resource = ['blueprint-1', 1],
      source = 'base',
      ...rest
    }) =>
      useEditorDraft({
        editor,
        ready,
        resource,
        schema: definitionDraftSchema,
        source,
        ...rest,
      }),
    { initialProps, wrapper },
  );
};

const storedValue = (key: string) =>
  JSON.parse(sessionStorage.getItem(key) ?? 'null')?.value;

beforeEach(() => {
  vi.useFakeTimers();
});

afterEach(() => {
  vi.useRealTimers();
  sessionStorage.clear();
});

describe('useEditorDraft', () => {
  it('debounces writes of dirty values and removes clean ones', () => {
    const key = storageKey(['blueprint-1', 1]);
    const { rerender } = renderDraft({ dirty: true, value: 'edit 1' });
    rerender({ dirty: true, value: 'edit 2' });
    expect(sessionStorage.getItem(key)).toBeNull();
    act(() => vi.advanceTimersByTime(draftWriteDelayMs));
    expect(storedValue(key)).toBe('edit 2');

    rerender({ dirty: false, value: 'base' });
    expect(sessionStorage.getItem(key)).toBeNull();
  });

  it('flushes a pending write when the page is hidden or unmounted', () => {
    const key = storageKey(['blueprint-1', 1]);
    const { rerender, unmount } = renderDraft({ dirty: true, value: 'edit' });
    act(() => window.dispatchEvent(new Event('pagehide')));
    expect(storedValue(key)).toBe('edit');

    rerender({ dirty: true, value: 'later edit' });
    unmount();
    expect(storedValue(key)).toBe('later edit');
  });

  it('waits until the source form is ready', () => {
    const key = storageKey(['blueprint-1', 1]);
    writeDraft(key, {
      savedAt: new Date().toISOString(),
      source: 'base',
      value: 'draft',
    });
    const { result, rerender } = renderDraft({
      dirty: false,
      ready: false,
      value: '',
    });
    expect(result.current.pending).toBeUndefined();
    rerender({ dirty: false, value: 'base' });
    expect(result.current.pending?.value).toBe('draft');
  });

  it('offers a stored draft without overwriting it until restored', () => {
    const key = storageKey(['blueprint-1', 1]);
    writeDraft(key, {
      savedAt: new Date().toISOString(),
      source: 'base',
      value: 'draft',
    });
    const { result, rerender } = renderDraft({ dirty: false, value: 'base' });

    expect(result.current.pending).toMatchObject({
      sourceChanged: false,
      value: 'draft',
    });
    act(() => vi.advanceTimersByTime(draftWriteDelayMs));
    expect(storedValue(key)).toBe('draft');

    let restored: string | undefined;
    act(() => {
      restored = result.current.restore();
    });
    expect(restored).toBe('draft');
    rerender({ dirty: true, value: 'draft' });
    expect(result.current.pending).toBeUndefined();
    expect(storedValue(key)).toBe('draft');
  });

  it('discards a stored draft without applying it', () => {
    const key = storageKey(['blueprint-1', 1]);
    writeDraft(key, {
      savedAt: new Date().toISOString(),
      source: 'base',
      value: 'draft',
    });
    const { result } = renderDraft({ dirty: false, value: 'base' });
    act(() => result.current.discard());
    expect(result.current.pending).toBeUndefined();
    expect(sessionStorage.getItem(key)).toBeNull();
  });

  it('flags a draft captured from a different source', () => {
    writeDraft(storageKey(['blueprint-1', 1]), {
      savedAt: new Date().toISOString(),
      source: 'old source',
      value: 'draft',
    });
    const { result } = renderDraft({ dirty: false, value: 'base' });
    expect(result.current.pending?.sourceChanged).toBe(true);
  });

  it('keeps drafts separate per resource context and user', () => {
    writeDraft(storageKey(['blueprint-1', 1]), {
      savedAt: new Date().toISOString(),
      source: 'base',
      value: 'draft',
    });
    const otherVersion = renderDraft({
      dirty: false,
      resource: ['blueprint-1', 2],
      value: 'base',
    });
    expect(otherVersion.result.current.pending).toBeUndefined();
    const otherUser = renderDraft({ dirty: false, value: 'base' }, 'user-2');
    expect(otherUser.result.current.pending).toBeUndefined();
  });

  it('stops persisting after a confirmed save clears the draft', () => {
    const key = storageKey(['blueprint-1', 1]);
    const { result, rerender, unmount } = renderDraft({
      dirty: true,
      value: 'edit',
    });
    act(() => result.current.clear());
    rerender({ dirty: true, value: 'edit' });
    act(() => vi.advanceTimersByTime(draftWriteDelayMs));
    unmount();
    expect(sessionStorage.getItem(key)).toBeNull();
  });
});
