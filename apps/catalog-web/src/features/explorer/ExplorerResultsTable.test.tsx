// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { ToastProvider } from '../../components/ToastProvider';
import type { BlueprintWithAttributes, EntityItem } from '../entities/api';
import { extensionQueryKeys } from '../extensions/query-keys';
import { ExplorerResultsTable } from './ExplorerResultsTable';

vi.mock('@tanstack/react-router', () => ({
  Link: ({ children }: { children: React.ReactNode }) => (
    <span>{children}</span>
  ),
}));
vi.mock('@tanstack/react-virtual', () => ({
  useVirtualizer: ({ count }: { count: number }) => ({
    getTotalSize: () => count * 53,
    getVirtualItems: () =>
      Array.from({ length: count }, (_, index) => ({
        end: (index + 1) * 53,
        index,
        key: index,
        size: 53,
        start: index * 53,
      })),
    measureElement: vi.fn(),
  }),
}));
vi.mock('../extensions/api', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../extensions/api')>()),
  getExtensionRuntime: vi.fn().mockResolvedValue({ contributions: [] }),
}));
vi.mock('../extensions/ExtensionOutlet', () => ({
  ExtensionPopoverOutlet: () => null,
}));

const blueprint = {
  blueprint: {
    code: 'product',
    id: '123e4567-e89b-12d3-a456-426614174000',
    name: 'Product',
    status: 'published',
    version: 1,
    views: {},
  },
  attributes: [],
  table_path_attributes: [],
} as unknown as BlueprintWithAttributes;

const item: EntityItem = {
  blueprint_version: 1,
  display: { default: 'Sample product' },
  id: '123e4567-e89b-12d3-a456-426614174001',
  is_sample: true,
  match_explanations: [],
  preview: {},
  related: {},
  schema_outdated: false,
  table_values: {},
};

describe('ExplorerResultsTable', () => {
  it('shows the localized Sample badge for a sample result', () => {
    const queryClient = new QueryClient({
      defaultOptions: { queries: { retry: false, staleTime: Infinity } },
    });
    queryClient.setQueryData(extensionQueryKeys.runtime(), {
      contributions: [],
    });
    render(
      <QueryClientProvider client={queryClient}>
        <ToastProvider>
          <ExplorerResultsTable
            blueprint={blueprint}
            canPublish={false}
            hasNextPage={false}
            isFetching={false}
            isFetchingNextPage={false}
            items={[item]}
            onLoadMore={vi.fn()}
            onSortChange={vi.fn()}
            publicationContextCode="default"
            publicationContextId={undefined}
            totalCount={1}
            totalCountCapped={false}
          />
        </ToastProvider>
      </QueryClientProvider>,
    );

    expect(screen.getByText('Sample')).toBeTruthy();
  });
});
