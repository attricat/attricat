// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { renderHook, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import { expect, it, vi } from 'vitest';
import { searchRecords, type Blueprint } from '../records/api';
import { overviewConcurrentSearches } from './constants';
import { useExplorerOverview } from './useExplorerOverview';

vi.mock('../records/api', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../records/api')>()),
  searchRecords: vi.fn(),
}));

const blueprints = Array.from(
  { length: overviewConcurrentSearches + 2 },
  (_, index): Blueprint => ({
    code: `blueprint_${index}`,
    id: crypto.randomUUID(),
    name: `Blueprint ${index}`,
    status: 'published',
    version: 1,
    views: {},
  }),
);

it('sends a limited number of overview searches at once', async () => {
  const running: Array<() => void> = [];
  vi.mocked(searchRecords).mockImplementation(
    () =>
      new Promise((resolve) => {
        running.push(() => resolve({ items: [] } as never));
      }),
  );
  const client = new QueryClient();
  const wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={client}>{children}</QueryClientProvider>
  );
  const { result } = renderHook(
    () =>
      useExplorerOverview(blueprints, { contextCode: 'default', query: '' }),
    { wrapper },
  );

  await waitFor(() =>
    expect(searchRecords).toHaveBeenCalledTimes(overviewConcurrentSearches),
  );
  running.shift()!();
  await waitFor(() =>
    expect(searchRecords).toHaveBeenCalledTimes(overviewConcurrentSearches + 1),
  );
  while (running.length) running.shift()!();
  await waitFor(() =>
    expect(searchRecords).toHaveBeenCalledTimes(blueprints.length),
  );
  while (running.length) running.shift()!();
  await waitFor(() =>
    expect(result.current.every((query) => query.isSuccess)).toBe(true),
  );
});
