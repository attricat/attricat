// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { renderHook, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import '../../i18n';
import {
  getBlueprintRevision,
  getEntityForm,
  getResolvedEntityPreview,
  getStatusTransitions,
} from './api';
import { useEntityPreviewData } from './useEntityPreviewData';

vi.mock('./api', async (importOriginal) => ({
  ...(await importOriginal<typeof import('./api')>()),
  getBlueprintRevision: vi.fn(),
  getEntityForm: vi.fn(),
  getResolvedEntityPreview: vi.fn(),
  getStatusTransitions: vi.fn(),
}));

const entityId = '123e4567-e89b-12d3-a456-426614174010';
const contextId = '123e4567-e89b-12d3-a456-426614174001';
const blueprintId = '123e4567-e89b-12d3-a456-426614174000';

const wrapper = ({ children }: { children: ReactNode }) => (
  <QueryClientProvider
    client={new QueryClient({ defaultOptions: { queries: { retry: false } } })}
  >
    {children}
  </QueryClientProvider>
);

const mockEntity = (canWrite: boolean) => {
  vi.mocked(getResolvedEntityPreview).mockResolvedValue({
    entity: { id: entityId, blueprint_id: blueprintId, blueprint_version: 3 },
    values: {},
  } as never);
  vi.mocked(getEntityForm).mockResolvedValue({ can_write: canWrite } as never);
  vi.mocked(getBlueprintRevision).mockResolvedValue({
    attributes: [],
  } as never);
  vi.mocked(getStatusTransitions).mockResolvedValue([] as never);
};

describe('useEntityPreviewData', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('loads the revision the resolved values belong to', async () => {
    mockEntity(true);
    const { result } = renderHook(
      () => useEntityPreviewData(entityId, contextId),
      { wrapper },
    );

    await waitFor(() => expect(result.current.blueprint.isSuccess).toBe(true));
    expect(getBlueprintRevision).toHaveBeenCalledWith(blueprintId, 3);
    await waitFor(() =>
      expect(result.current.statusTransitions.isSuccess).toBe(true),
    );
    expect(vi.mocked(getStatusTransitions).mock.calls[0]?.slice(0, 2)).toEqual([
      entityId,
      contextId,
    ]);
  });

  it('asks for status transitions only when the user can edit', async () => {
    mockEntity(false);
    const { result } = renderHook(
      () => useEntityPreviewData(entityId, contextId),
      { wrapper },
    );

    await waitFor(() => expect(result.current.entityForm.isSuccess).toBe(true));
    expect(result.current.statusTransitions.fetchStatus).toBe('idle');
    expect(getStatusTransitions).not.toHaveBeenCalled();
  });

  it('waits for a context before resolving values', () => {
    mockEntity(true);
    const { result } = renderHook(() => useEntityPreviewData(entityId, null), {
      wrapper,
    });

    expect(result.current.resolved.fetchStatus).toBe('idle');
    expect(result.current.blueprint.fetchStatus).toBe('idle');
    expect(getResolvedEntityPreview).not.toHaveBeenCalled();
  });
});
