// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { act, renderHook, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { ApiRequestError } from '../../api/request';
import { updateRecord, type Attribute } from './api';
import { fieldSaveRequest, schemaMismatchField } from './recordFieldSaves';
import { useRecordFieldSaves } from './useRecordFieldSaves';

vi.mock('./api', async (importOriginal) => ({
  ...(await importOriginal<typeof import('./api')>()),
  updateRecord: vi.fn(),
}));

const recordId = '123e4567-e89b-12d3-a456-426614174000';
const targetId = '123e4567-e89b-12d3-a456-426614174001';
const attributes = [
  { code: 'title', value_type: 'string' },
  { code: 'summary', value_type: 'string' },
  { code: 'related', value_type: 'relationship' },
  { code: 'manual', value_type: 'file' },
] satisfies Attribute[];

const savedRecord = (updatedAt: string) => ({
  id: recordId,
  updated_at: updatedAt,
});

const deferred = <T,>() => {
  let resolve!: (value: T) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
};

const renderSaves = (
  savedFields: Record<string, string> = {},
  updatedAt = 'v1',
) => {
  const client = new QueryClient();
  const wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={client}>{children}</QueryClientProvider>
  );
  return renderHook(
    () =>
      useRecordFieldSaves({
        recordId,
        contextId: null,
        attributes,
        savedFields,
        updatedAt,
      }),
    { wrapper },
  );
};

describe('fieldSaveRequest', () => {
  it('sends only changed attributes and removes cleared scalars', () => {
    expect(
      fieldSaveRequest(
        attributes,
        { title: 'New', summary: '', related: '' },
        { title: 'Old', summary: 'Saved', related: targetId },
        null,
      ),
    ).toEqual({
      values: [
        expect.objectContaining({ attribute_code: 'title', value: 'New' }),
      ],
      relationships: [
        { attribute_code: 'related', context_id: null, target_record_ids: [] },
      ],
      remove_values: [{ attribute_code: 'summary', context_id: null }],
    });
  });

  it('never removes a scalar that was not saved and skips files', () => {
    expect(
      fieldSaveRequest(attributes, { summary: '', manual: 'x' }, {}, null),
    ).toEqual({ values: [], relationships: [], remove_values: [] });
  });
});

describe('schemaMismatchField', () => {
  const mismatch = (details: unknown, message = 'Bad') =>
    new ApiRequestError(422, message, 'record_schema_mismatch', details);

  it('finds a missing required property in the server message', () => {
    expect(
      schemaMismatchField(
        mismatch(
          { context: 'default', instance_path: '' },
          "resolved record values for context 'default' do not match the record schema at '': \"name\" is a required property",
        ),
        ['name'],
      ),
    ).toEqual({ code: 'name', missing: true });
  });

  it('uses the instance path and ignores fields not on the page', () => {
    expect(
      schemaMismatchField(mismatch({ instance_path: '/price' }, 'too small'), [
        'price',
      ]),
    ).toEqual({ code: 'price', missing: false });
    expect(
      schemaMismatchField(mismatch({ instance_path: '/hidden' }), ['price']),
    ).toBeUndefined();
    expect(schemaMismatchField(new Error('x'), ['price'])).toBeUndefined();
  });
});

describe('useRecordFieldSaves', () => {
  beforeEach(() => vi.mocked(updateRecord).mockReset());

  it('saves each commit against the version the previous save produced', async () => {
    const first = deferred<ReturnType<typeof savedRecord>>();
    vi.mocked(updateRecord)
      .mockReturnValueOnce(first.promise as never)
      .mockResolvedValueOnce(savedRecord('v3') as never);
    const { result } = renderSaves({ title: 'Old' });

    act(() => result.current.commit('title', 'A'));
    act(() => result.current.commit('summary', 'B'));
    expect(updateRecord).toHaveBeenCalledTimes(1);
    expect(result.current.fields).toMatchObject({ title: 'A', summary: 'B' });

    await act(async () => first.resolve(savedRecord('v2')));
    await waitFor(() => expect(result.current.saving).toBe(false));

    expect(updateRecord).toHaveBeenCalledTimes(2);
    expect(vi.mocked(updateRecord).mock.calls[0]![1]).toMatchObject({
      expected_updated_at: 'v1',
      values: [expect.objectContaining({ attribute_code: 'title' })],
    });
    expect(vi.mocked(updateRecord).mock.calls[1]![1]).toMatchObject({
      expected_updated_at: 'v2',
      values: [expect.objectContaining({ attribute_code: 'summary' })],
    });
    expect(result.current.pending).toEqual({});
  });

  it('keeps a rejected change pending and resends it with the next commit', async () => {
    vi.mocked(updateRecord)
      .mockRejectedValueOnce(
        new ApiRequestError(422, 'Missing', 'record_schema_mismatch'),
      )
      .mockResolvedValueOnce(savedRecord('v2') as never);
    const { result } = renderSaves();

    act(() => result.current.commit('title', 'A'));
    await waitFor(() => expect(result.current.error).not.toBeNull());
    expect(result.current.pending).toEqual({ title: 'A' });

    act(() => result.current.commit('summary', 'B'));
    await waitFor(() => expect(result.current.pending).toEqual({}));
    expect(vi.mocked(updateRecord).mock.calls[1]![1]).toMatchObject({
      expected_updated_at: 'v1',
      values: [
        expect.objectContaining({ attribute_code: 'title' }),
        expect.objectContaining({ attribute_code: 'summary' }),
      ],
    });
    expect(result.current.error).toBeNull();
  });

  it('stops on another editor’s change until the conflict is resolved', async () => {
    vi.mocked(updateRecord)
      .mockRejectedValueOnce(new ApiRequestError(409, 'Stale', 'stale_record'))
      .mockResolvedValueOnce(savedRecord('v3') as never);
    const { result } = renderSaves({ title: 'Old' });

    act(() => result.current.commit('title', 'Mine'));
    await waitFor(() => expect(result.current.conflict).toBe(true));
    act(() => result.current.commit('summary', 'Also mine'));
    expect(updateRecord).toHaveBeenCalledTimes(1);

    act(() =>
      result.current.resolveConflict('keepMine', {
        savedFields: { title: 'Theirs', summary: 'Also mine' },
        updatedAt: 'v2',
      }),
    );
    await waitFor(() => expect(result.current.pending).toEqual({}));
    expect(vi.mocked(updateRecord).mock.calls[1]![1]).toMatchObject({
      expected_updated_at: 'v2',
      values: [expect.objectContaining({ attribute_code: 'title' })],
    });
    expect(result.current.fields).toMatchObject({
      title: 'Mine',
      summary: 'Also mine',
    });
  });

  it('discards pending changes when taking the other editor’s values', async () => {
    vi.mocked(updateRecord).mockRejectedValueOnce(
      new ApiRequestError(409, 'Stale', 'stale_record'),
    );
    const { result } = renderSaves({ title: 'Old' });

    act(() => result.current.commit('title', 'Mine'));
    await waitFor(() => expect(result.current.conflict).toBe(true));
    act(() =>
      result.current.resolveConflict('useTheirs', {
        savedFields: { title: 'Theirs' },
        updatedAt: 'v2',
      }),
    );

    expect(result.current.fields).toEqual({ title: 'Theirs' });
    expect(updateRecord).toHaveBeenCalledTimes(1);
  });

  it('adopts a newer server version while nothing is pending', async () => {
    vi.mocked(updateRecord).mockResolvedValue(savedRecord('v3') as never);
    const client = new QueryClient();
    const { result, rerender } = renderHook(
      ({ updatedAt, title }: { updatedAt: string; title: string }) =>
        useRecordFieldSaves({
          recordId,
          contextId: null,
          attributes,
          savedFields: { title },
          updatedAt,
        }),
      {
        initialProps: { updatedAt: '2026-10-07T10:00:00Z', title: 'Old' },
        wrapper: ({ children }: { children: ReactNode }) => (
          <QueryClientProvider client={client}>{children}</QueryClientProvider>
        ),
      },
    );

    // An older refetch never replaces what this editor saved.
    rerender({ updatedAt: '2026-10-07T09:00:00Z', title: 'Stale' });
    expect(result.current.fields).toEqual({ title: 'Old' });

    rerender({
      updatedAt: '2026-10-07T10:05:00Z',
      title: 'Attached elsewhere',
    });
    expect(result.current.fields).toEqual({ title: 'Attached elsewhere' });

    act(() => result.current.commit('summary', 'B'));
    await waitFor(() => expect(result.current.pending).toEqual({}));
    expect(vi.mocked(updateRecord).mock.calls[0]![1]).toMatchObject({
      expected_updated_at: '2026-10-07T10:05:00Z',
    });
  });

  it('reports when changes start and stop waiting to be saved', async () => {
    vi.mocked(updateRecord).mockResolvedValue(savedRecord('v2') as never);
    const onPendingChange = vi.fn();
    const client = new QueryClient();
    const { result } = renderHook(
      () =>
        useRecordFieldSaves({
          recordId,
          contextId: null,
          attributes,
          savedFields: {},
          updatedAt: 'v1',
          onPendingChange,
        }),
      {
        wrapper: ({ children }: { children: ReactNode }) => (
          <QueryClientProvider client={client}>{children}</QueryClientProvider>
        ),
      },
    );

    act(() => result.current.commit('title', 'A'));
    await waitFor(() => expect(result.current.pending).toEqual({}));
    expect(onPendingChange.mock.calls).toEqual([[true], [false]]);
  });

  it('resends a save made stale by this editor’s own file change', async () => {
    const first = deferred<never>();
    vi.mocked(updateRecord)
      .mockReturnValueOnce(first.promise)
      .mockResolvedValueOnce(savedRecord('2026-10-07T10:02:00Z') as never);
    const { result } = renderSaves({ title: 'Old' }, '2026-10-07T10:00:00Z');

    act(() => result.current.commit('title', 'A'));
    act(() => result.current.noteRecordUpdated('2026-10-07T10:01:00Z'));
    await act(async () =>
      first.reject(new ApiRequestError(409, 'Stale', 'stale_record')),
    );
    await waitFor(() => expect(result.current.pending).toEqual({}));

    expect(result.current.conflict).toBe(false);
    expect(vi.mocked(updateRecord).mock.calls[1]![1]).toMatchObject({
      expected_updated_at: '2026-10-07T10:01:00Z',
    });
  });

  it('never moves the version back for a late or older result', async () => {
    const first = deferred<ReturnType<typeof savedRecord>>();
    vi.mocked(updateRecord)
      .mockReturnValueOnce(first.promise as never)
      .mockResolvedValueOnce(savedRecord('2026-10-07T10:05:00Z') as never);
    const { result } = renderSaves({ title: 'Old' }, '2026-10-07T10:00:00Z');

    act(() => result.current.commit('title', 'A'));
    act(() => result.current.noteRecordUpdated('2026-10-07T10:03:00Z'));
    await act(async () => first.resolve(savedRecord('2026-10-07T10:02:00Z')));
    act(() => result.current.noteRecordUpdated('2026-10-07T10:01:00Z'));

    act(() => result.current.commit('summary', 'B'));
    await waitFor(() => expect(result.current.pending).toEqual({}));
    expect(vi.mocked(updateRecord).mock.calls[1]![1]).toMatchObject({
      expected_updated_at: '2026-10-07T10:03:00Z',
    });
  });

  it('clears the failure once its change is discarded', async () => {
    vi.mocked(updateRecord).mockRejectedValueOnce(new Error('Offline'));
    const { result } = renderSaves({ title: 'Old' });

    act(() => result.current.commit('title', 'A'));
    await waitFor(() => expect(result.current.error).not.toBeNull());
    act(() => result.current.revert('title'));

    expect(result.current.error).toBeNull();
    expect(result.current.fields).toEqual({ title: 'Old' });
  });

  it('does not save a value that returns to the saved one', () => {
    const { result } = renderSaves({ title: 'Old' });
    act(() => result.current.commit('title', 'Old'));
    expect(updateRecord).not.toHaveBeenCalled();
  });
});
