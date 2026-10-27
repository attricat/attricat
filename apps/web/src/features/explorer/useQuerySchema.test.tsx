// @vitest-environment jsdom
import type { ReactNode } from 'react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { renderHook, waitFor } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { getBlueprintByCode } from '../records/api';
import type { BlueprintWithAttributes } from '../records/api';
import { useQuerySchema } from './useQuerySchema';

vi.mock('../records/api', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../records/api')>()),
  getBlueprintByCode: vi.fn(),
}));

const selected = {
  blueprint: {
    id: '123e4567-e89b-12d3-a456-426614174000',
    code: 'product',
    name: 'Product',
    status: 'published',
    version: 1,
    views: {},
  },
  attributes: [
    { code: 'title', value_type: 'string' },
    {
      code: 'maker',
      value_type: 'relationship',
      target_blueprint_code: 'company',
    },
  ],
  table_path_attributes: [],
} as BlueprintWithAttributes;

describe('useQuerySchema', () => {
  it('keeps the schema while nothing it is built from changes', () => {
    const client = new QueryClient();
    const wrapper = ({ children }: { children: ReactNode }) => (
      <QueryClientProvider client={client}>{children}</QueryClientProvider>
    );
    const { result, rerender } = renderHook(
      ({ query }) => useQuerySchema(selected, [], query),
      { initialProps: { query: 'title' }, wrapper },
    );
    const first = result.current.schema;
    expect(first?.blueprint.code).toBe('product');
    rerender({ query: 'title:' });
    expect(result.current.schema).toBe(first);
  });

  it('reports a relationship target that failed to load', async () => {
    const failure = new Error('Target failed');
    vi.mocked(getBlueprintByCode).mockRejectedValue(failure);
    const client = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    });
    const wrapper = ({ children }: { children: ReactNode }) => (
      <QueryClientProvider client={client}>{children}</QueryClientProvider>
    );
    const { result } = renderHook(
      () => useQuerySchema(selected, [], 'maker.'),
      { wrapper },
    );
    await waitFor(() => expect(result.current.targetsError).toBe(failure));
    expect(result.current.targetsLoading).toBe(false);
  });
});
