// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { renderHook, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import { expect, it, vi } from 'vitest';
import { getEntityLabels } from './api';
import { useEntityLabels } from './useEntityLabels';

vi.mock('./api', () => ({ getEntityLabels: vi.fn() }));

const entityId = (index: number) =>
  `00000000-0000-4000-8000-${String(index).padStart(12, '0')}`;

it('looks labels up in batches of at most 100 distinct IDs', async () => {
  vi.mocked(getEntityLabels).mockImplementation((ids) =>
    Promise.resolve({
      items: ids.map((id) => ({
        id,
        blueprint_code: 'product',
        display: { default: id === entityId(0) ? '' : `Label ${id}` },
      })),
    }),
  );
  const ids = Array.from({ length: 150 }, (_, index) => entityId(index));
  const client = new QueryClient();
  const wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={client}>{children}</QueryClientProvider>
  );
  const { result } = renderHook(
    () => useEntityLabels([...ids, ...ids.slice(0, 10)]),
    { wrapper },
  );

  await waitFor(() => expect(result.current.size).toBe(149));
  expect(getEntityLabels).toHaveBeenCalledTimes(2);
  const batches = vi.mocked(getEntityLabels).mock.calls.map(([batch]) => batch);
  expect(batches.map((batch) => batch.length)).toEqual([100, 50]);
  // An empty label falls back to the caller's own rendering.
  expect(result.current.has(entityId(0))).toBe(false);
  expect(result.current.get(entityId(1))).toBe(`Label ${entityId(1)}`);
});
