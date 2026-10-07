// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { act, renderHook, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { ApiRequestError } from '../../api/request';
import { updateEntity, type Attribute } from './api';
import { fieldSaveRequest, schemaMismatchField } from './entityFieldSaves';
import { useEntityFieldSaves } from './useEntityFieldSaves';

vi.mock('./api', async (importOriginal) => ({
  ...(await importOriginal<typeof import('./api')>()),
  updateEntity: vi.fn(),
}));

const entityId = '123e4567-e89b-12d3-a456-426614174000';
const targetId = '123e4567-e89b-12d3-a456-426614174001';
const attributes = [
  { code: 'title', value_type: 'string' },
  { code: 'summary', value_type: 'string' },
  { code: 'related', value_type: 'relationship' },
  { code: 'manual', value_type: 'file' },
] satisfies Attribute[];

const savedEntity = (updatedAt: string) => ({
  id: entityId,
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

const renderSaves = (savedFields: Record<string, string> = {}) => {
  const client = new QueryClient();
  const wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={client}>{children}</QueryClientProvider>
  );
  return renderHook(
    () =>
      useEntityFieldSaves({
        entityId,
        contextId: null,
        attributes,
        savedFields,
        updatedAt: 'v1',
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
        { attribute_code: 'related', context_id: null, target_entity_ids: [] },
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
    new ApiRequestError(422, message, 'entity_schema_mismatch', details);

  it('finds a missing required property in the server message', () => {
    expect(
      schemaMismatchField(
        mismatch(
          { context: 'default', instance_path: '' },
          "resolved entity values for context 'default' do not match the entity schema at '': \"name\" is a required property",
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

describe('useEntityFieldSaves', () => {
  beforeEach(() => vi.mocked(updateEntity).mockReset());

  it('saves each commit against the version the previous save produced', async () => {
    const first = deferred<ReturnType<typeof savedEntity>>();
    vi.mocked(updateEntity)
      .mockReturnValueOnce(first.promise as never)
      .mockResolvedValueOnce(savedEntity('v3') as never);
    const { result } = renderSaves({ title: 'Old' });

    act(() => result.current.commit('title', 'A'));
    act(() => result.current.commit('summary', 'B'));
    expect(updateEntity).toHaveBeenCalledTimes(1);
    expect(result.current.fields).toMatchObject({ title: 'A', summary: 'B' });

    await act(async () => first.resolve(savedEntity('v2')));
    await waitFor(() => expect(result.current.saving).toBe(false));

    expect(updateEntity).toHaveBeenCalledTimes(2);
    expect(vi.mocked(updateEntity).mock.calls[0]![1]).toMatchObject({
      expected_updated_at: 'v1',
      values: [expect.objectContaining({ attribute_code: 'title' })],
    });
    expect(vi.mocked(updateEntity).mock.calls[1]![1]).toMatchObject({
      expected_updated_at: 'v2',
      values: [expect.objectContaining({ attribute_code: 'summary' })],
    });
    expect(result.current.pending).toEqual({});
  });

  it('keeps a rejected change pending and resends it with the next commit', async () => {
    vi.mocked(updateEntity)
      .mockRejectedValueOnce(
        new ApiRequestError(422, 'Missing', 'entity_schema_mismatch'),
      )
      .mockResolvedValueOnce(savedEntity('v2') as never);
    const { result } = renderSaves();

    act(() => result.current.commit('title', 'A'));
    await waitFor(() => expect(result.current.error).not.toBeNull());
    expect(result.current.pending).toEqual({ title: 'A' });

    act(() => result.current.commit('summary', 'B'));
    await waitFor(() => expect(result.current.pending).toEqual({}));
    expect(vi.mocked(updateEntity).mock.calls[1]![1]).toMatchObject({
      expected_updated_at: 'v1',
      values: [
        expect.objectContaining({ attribute_code: 'title' }),
        expect.objectContaining({ attribute_code: 'summary' }),
      ],
    });
    expect(result.current.error).toBeNull();
  });

  it('stops on another editor’s change until the conflict is resolved', async () => {
    vi.mocked(updateEntity)
      .mockRejectedValueOnce(new ApiRequestError(409, 'Stale', 'stale_entity'))
      .mockResolvedValueOnce(savedEntity('v3') as never);
    const { result } = renderSaves({ title: 'Old' });

    act(() => result.current.commit('title', 'Mine'));
    await waitFor(() => expect(result.current.conflict).toBe(true));
    act(() => result.current.commit('summary', 'Also mine'));
    expect(updateEntity).toHaveBeenCalledTimes(1);

    act(() =>
      result.current.resolveConflict('keepMine', {
        savedFields: { title: 'Theirs', summary: 'Also mine' },
        updatedAt: 'v2',
      }),
    );
    await waitFor(() => expect(result.current.pending).toEqual({}));
    expect(vi.mocked(updateEntity).mock.calls[1]![1]).toMatchObject({
      expected_updated_at: 'v2',
      values: [expect.objectContaining({ attribute_code: 'title' })],
    });
    expect(result.current.fields).toMatchObject({
      title: 'Mine',
      summary: 'Also mine',
    });
  });

  it('discards pending changes when taking the other editor’s values', async () => {
    vi.mocked(updateEntity).mockRejectedValueOnce(
      new ApiRequestError(409, 'Stale', 'stale_entity'),
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
    expect(updateEntity).toHaveBeenCalledTimes(1);
  });

  it('does not save a value that returns to the saved one', () => {
    const { result } = renderSaves({ title: 'Old' });
    act(() => result.current.commit('title', 'Old'));
    expect(updateEntity).not.toHaveBeenCalled();
  });
});
