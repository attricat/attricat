// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { renderHook, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import '../../i18n';
import {
  getBlueprintRevision,
  getRecordForm,
  getResolvedRecordPreview,
  getStatusTransitions,
} from './api';
import { useRecordPreviewData } from './useRecordPreviewData';

vi.mock('./api', async (importOriginal) => ({
  ...(await importOriginal<typeof import('./api')>()),
  getBlueprintRevision: vi.fn(),
  getRecordForm: vi.fn(),
  getResolvedRecordPreview: vi.fn(),
  getStatusTransitions: vi.fn(),
}));

const recordId = '123e4567-e89b-12d3-a456-426614174010';
const contextId = '123e4567-e89b-12d3-a456-426614174001';
const blueprintId = '123e4567-e89b-12d3-a456-426614174000';

const wrapper = ({ children }: { children: ReactNode }) => (
  <QueryClientProvider
    client={new QueryClient({ defaultOptions: { queries: { retry: false } } })}
  >
    {children}
  </QueryClientProvider>
);

const revision = { blueprint: { id: blueprintId, version: 3 }, attributes: [] };

const later = <T,>(value: T) =>
  new Promise<T>((resolve) => setTimeout(() => resolve(value), 20));

const mockRecord = (
  canWrite: boolean,
  { formFirst = false }: { formFirst?: boolean } = {},
) => {
  const resolved = {
    record: { id: recordId, blueprint_id: blueprintId, blueprint_version: 3 },
    values: {},
  } as never;
  const form = { can_write: canWrite, blueprint: revision } as never;
  vi.mocked(getResolvedRecordPreview).mockImplementation(() =>
    formFirst ? later(resolved) : Promise.resolve(resolved),
  );
  vi.mocked(getRecordForm).mockImplementation(() =>
    formFirst ? Promise.resolve(form) : later(form),
  );
  vi.mocked(getBlueprintRevision).mockResolvedValue(revision as never);
  vi.mocked(getStatusTransitions).mockResolvedValue([] as never);
};

describe('useRecordPreviewData', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('loads the revision the resolved values belong to', async () => {
    mockRecord(true);
    const { result } = renderHook(
      () => useRecordPreviewData(recordId, contextId),
      { wrapper },
    );

    await waitFor(() => expect(result.current.blueprint.isSuccess).toBe(true));
    expect(getBlueprintRevision).toHaveBeenCalledWith(blueprintId, 3);
    await waitFor(() =>
      expect(result.current.statusTransitions.isSuccess).toBe(true),
    );
    expect(vi.mocked(getStatusTransitions).mock.calls[0]?.slice(0, 2)).toEqual([
      recordId,
      contextId,
    ]);
  });

  it('reuses the revision the form carries when it arrives first', async () => {
    mockRecord(true, { formFirst: true });
    const { result } = renderHook(
      () => useRecordPreviewData(recordId, contextId),
      { wrapper },
    );

    await waitFor(() => expect(result.current.blueprint.isSuccess).toBe(true));
    expect(result.current.blueprint.data).toEqual(revision);
    expect(getBlueprintRevision).not.toHaveBeenCalled();
  });

  it('asks for status transitions only when the user can edit', async () => {
    mockRecord(false);
    const { result } = renderHook(
      () => useRecordPreviewData(recordId, contextId),
      { wrapper },
    );

    await waitFor(() => expect(result.current.recordForm.isSuccess).toBe(true));
    expect(result.current.statusTransitions.fetchStatus).toBe('idle');
    expect(getStatusTransitions).not.toHaveBeenCalled();
  });

  it('waits for a context before resolving values', () => {
    mockRecord(true);
    const { result } = renderHook(() => useRecordPreviewData(recordId, null), {
      wrapper,
    });

    expect(result.current.resolved.fetchStatus).toBe('idle');
    expect(result.current.blueprint.fetchStatus).toBe('idle');
    expect(getResolvedRecordPreview).not.toHaveBeenCalled();
  });
});
