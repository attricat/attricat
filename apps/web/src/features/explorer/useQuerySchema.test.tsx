// @vitest-environment jsdom
import type { ReactNode } from 'react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { renderHook } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import type { BlueprintWithAttributes } from '../records/api';
import { useQuerySchema } from './useQuerySchema';

const selected = {
  blueprint: {
    id: '123e4567-e89b-12d3-a456-426614174000',
    code: 'product',
    name: 'Product',
    status: 'published',
    version: 1,
    views: {},
  },
  attributes: [{ code: 'title', value_type: 'string' }],
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
});
