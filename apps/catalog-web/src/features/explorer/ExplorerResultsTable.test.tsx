// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { ToastProvider } from '../../components/ToastProvider';
import type { BlueprintWithAttributes, EntityItem } from '../entities/api';
import { extensionQueryKeys } from '../extensions/query-keys';
import { ExplorerResultsTable } from './ExplorerResultsTable';

const navigate = vi.fn();
vi.mock('@tanstack/react-router', () => ({
  Link: ({ children }: { children: React.ReactNode }) => (
    <span>{children}</span>
  ),
  useNavigate: () => navigate,
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
vi.mock('../agents/api', () => ({
  createConversation: vi.fn(),
  sendMessage: vi.fn(),
}));

import { createConversation, sendMessage } from '../agents/api';

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
const secondItem: EntityItem = {
  ...item,
  id: '123e4567-e89b-12d3-a456-426614174002',
  display: { default: 'Second product' },
  is_sample: false,
};

const renderTable = (items = [item, secondItem]) => {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false, staleTime: Infinity } },
  });
  queryClient.setQueryData(extensionQueryKeys.runtime(), {
    contributions: [],
  });
  return render(
    <QueryClientProvider client={queryClient}>
      <ToastProvider>
        <ExplorerResultsTable
          blueprint={blueprint}
          canPublish={false}
          hasNextPage={false}
          isFetching={false}
          isFetchingNextPage={false}
          items={items}
          onLoadMore={vi.fn()}
          onSortChange={vi.fn()}
          publicationContextCode="default"
          publicationContextId={undefined}
          totalCount={items.length}
          totalCountCapped={false}
        />
      </ToastProvider>
    </QueryClientProvider>,
  );
};

describe('ExplorerResultsTable', () => {
  it('shows the localized Sample badge and hides selection by default', () => {
    renderTable([item]);
    expect(screen.getByText('Sample')).toBeTruthy();
    expect(screen.queryByRole('checkbox')).toBeNull();
  });

  it('selects individual and loaded rows, clears and exits selection mode', async () => {
    const user = userEvent.setup();
    renderTable();
    await user.click(screen.getByRole('button', { name: 'Select entities' }));
    const selectAll = screen.getByRole('checkbox', {
      name: 'Select loaded entities (up to 50)',
    }) as HTMLInputElement;
    const first = screen.getByRole('checkbox', {
      name: 'Select Sample product',
    });
    await user.click(first);
    expect((first as HTMLInputElement).checked).toBe(true);
    expect(selectAll.getAttribute('data-indeterminate')).toBe('true');
    expect(screen.getByText('1 selected')).toBeTruthy();
    await user.click(selectAll);
    expect(selectAll.checked).toBe(true);
    expect(screen.getByText('2 selected')).toBeTruthy();
    await user.click(screen.getByRole('button', { name: 'Clear' }));
    expect(selectAll.checked).toBe(false);
    await user.click(first);
    await user.click(screen.getByRole('button', { name: 'Exit selection' }));
    expect(screen.queryByRole('checkbox')).toBeNull();
    await user.click(screen.getByRole('button', { name: 'Select entities' }));
    expect(screen.queryByText('1 selected')).toBeNull();
  });

  it('limits bulk selection to 50 loaded entities', async () => {
    const user = userEvent.setup();
    const items = Array.from({ length: 51 }, (_, index) => ({
      ...item,
      id: `123e4567-e89b-12d3-a456-${String(index).padStart(12, '0')}`,
      display: { default: `Product ${index}` },
    }));
    renderTable(items);
    await user.click(screen.getByRole('button', { name: 'Select entities' }));
    await user.click(
      screen.getByRole('checkbox', {
        name: 'Select loaded entities (up to 50)',
      }),
    );
    expect(screen.getByText('50 selected')).toBeTruthy();
    expect(
      screen
        .getByRole('checkbox', { name: 'Select Product 50' })
        .hasAttribute('disabled'),
    ).toBe(true);
  });

  it('sends only selected entity references to a new conversation', async () => {
    const user = userEvent.setup();
    vi.mocked(createConversation).mockResolvedValue({
      id: '123e4567-e89b-12d3-a456-426614174003',
      title: 'Review',
    });
    vi.mocked(sendMessage).mockResolvedValue({} as never);
    renderTable();
    await user.click(screen.getByRole('button', { name: 'Select entities' }));
    await user.click(
      screen.getByRole('checkbox', { name: 'Select Sample product' }),
    );
    await user.click(
      screen.getByRole('button', { name: 'Send to agent conversation' }),
    );
    const dialog = screen.getByRole('dialog');
    await user.type(
      within(dialog).getByRole('textbox', {
        name: 'Instructions for the agent',
      }),
      ' Check prices.',
    );
    await user.click(
      within(dialog).getByRole('button', {
        name: 'Send to agent conversation',
      }),
    );
    await waitFor(() => expect(sendMessage).toHaveBeenCalledTimes(1));
    expect(createConversation).toHaveBeenCalledWith(
      'Review 1 Product entities',
    );
    expect(sendMessage).toHaveBeenCalledWith(
      '123e4567-e89b-12d3-a456-426614174003',
      expect.stringContaining(`entity_id: ${item.id}`),
    );
    expect(vi.mocked(sendMessage).mock.calls[0][1]).not.toContain(
      secondItem.id,
    );
    await waitFor(() =>
      expect(navigate).toHaveBeenCalledWith({
        to: '/agents/$conversationId',
        params: { conversationId: '123e4567-e89b-12d3-a456-426614174003' },
      }),
    );
  });

  it('retries a failed message without creating another conversation', async () => {
    const user = userEvent.setup();
    vi.mocked(createConversation).mockResolvedValue({
      id: '123e4567-e89b-12d3-a456-426614174004',
      title: 'Review',
    });
    vi.mocked(sendMessage)
      .mockRejectedValueOnce(new Error('Network error'))
      .mockResolvedValueOnce({} as never);
    renderTable([item]);
    await user.click(screen.getByRole('button', { name: 'Select entities' }));
    await user.click(
      screen.getByRole('checkbox', { name: 'Select Sample product' }),
    );
    await user.click(
      screen.getByRole('button', { name: 'Send to agent conversation' }),
    );
    const send = within(screen.getByRole('dialog')).getByRole('button', {
      name: 'Send to agent conversation',
    });
    await user.click(send);
    await screen.findByText('Network error');
    await user.click(send);
    await waitFor(() => expect(sendMessage).toHaveBeenCalledTimes(2));
    expect(createConversation).toHaveBeenCalledTimes(1);
  });
});
