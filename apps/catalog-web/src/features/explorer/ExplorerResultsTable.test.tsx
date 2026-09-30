// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import type { ComponentProps } from 'react';
import { describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { ToastProvider } from '../../components/ToastProvider';
import type { BlueprintWithAttributes, EntityItem } from '../entities/api';
import { extensionQueryKeys } from '../extensions/queryKeys';
import { ExplorerResultsTable } from './ExplorerResultsTable';
import { useExplorerSelection } from './useExplorerSelection';

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
vi.mock('../entities/api', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../entities/api')>()),
  deleteEntity: vi.fn(),
}));
vi.mock('../extensions/api', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../extensions/api')>()),
  getExtensionRuntime: vi.fn().mockResolvedValue({ contributions: [] }),
}));
const { outletMount } = vi.hoisted(() => ({ outletMount: vi.fn() }));
vi.mock('../extensions/ExtensionOutlet', () => ({
  ExtensionPopoverOutlet: () => null,
  ExtensionOutlet: (props: unknown) => {
    outletMount(props);
    return null;
  },
}));
vi.mock('../agents/api', () => ({
  createConversation: vi.fn(),
  sendMessage: vi.fn(),
}));

import { createConversation, sendMessage } from '../agents/api';
import { deleteEntity } from '../entities/api';

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

// Selection is owned above the table, which remounts for every search.
const TableWithSelection = ({
  tableKey,
  ...props
}: Omit<ComponentProps<typeof ExplorerResultsTable>, 'selection'> & {
  tableKey: string;
}) => {
  const selection = useExplorerSelection(
    props.blueprint.blueprint.code,
    props.items,
  );
  return (
    <ExplorerResultsTable key={tableKey} {...props} selection={selection} />
  );
};

const renderTable = (
  items = [item, secondItem],
  canDelete = false,
  onSortChange = vi.fn(),
  sort?: { field: string; direction: 'asc' | 'desc' },
  publicationSortAvailable = true,
  showExplorerActions = true,
  tableKey = 'initial',
) => {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false, staleTime: Infinity } },
  });
  queryClient.setQueryData(extensionQueryKeys.runtime(), {
    contributions: [],
  });
  const tree = (tableItems: EntityItem[], key: string) => (
    <QueryClientProvider client={queryClient}>
      <ToastProvider>
        <TableWithSelection
          tableKey={key}
          blueprint={blueprint}
          canPublish={false}
          canDelete={canDelete}
          hasNextPage={false}
          isFetching={false}
          isFetchingNextPage={false}
          items={tableItems}
          onLoadMore={vi.fn()}
          onSortChange={onSortChange}
          sort={sort}
          publicationContextCode="default"
          publicationContextId={undefined}
          publicationSortAvailable={publicationSortAvailable}
          showExplorerActions={showExplorerActions}
          totalCount={tableItems.length}
          totalCountCapped={false}
        />
      </ToastProvider>
    </QueryClientProvider>
  );
  const view = render(tree(items, tableKey));
  return {
    ...view,
    search: (nextItems: EntityItem[], key: string) =>
      view.rerender(tree(nextItems, key)),
  };
};

describe('ExplorerResultsTable', () => {
  it('offers revision-scoped Explorer actions only for single-version results', () => {
    outletMount.mockClear();
    renderTable([item], false, vi.fn(), undefined, true, false);
    expect(outletMount).not.toHaveBeenCalled();
    renderTable([item]);
    expect(outletMount).toHaveBeenCalledWith({
      outlet: 'explorer_action',
      context: {
        context_version: 1,
        blueprint_id: blueprint.blueprint.id,
        blueprint_version: blueprint.blueprint.version,
      },
      runtimeScope: {
        blueprintId: blueprint.blueprint.id,
        blueprintVersion: blueprint.blueprint.version,
      },
    });
  });

  it('sorts publication status for the selected channel', async () => {
    const onSortChange = vi.fn();
    renderTable([item], false, onSortChange);
    await userEvent
      .setup()
      .click(screen.getByRole('button', { name: /Publication/ }));
    expect(onSortChange).toHaveBeenCalledWith('publication_status');
  });

  it('does not offer publication sorting when the channel is unavailable', () => {
    renderTable([item], false, vi.fn(), undefined, false);
    expect(screen.queryByRole('button', { name: /Publication/ })).toBeNull();
  });

  it('offers a sortable schema header even without configured table columns', async () => {
    const onSortChange = vi.fn();
    renderTable([item], false, onSortChange, {
      field: 'blueprint_version',
      direction: 'asc',
    });
    const header = screen.getByRole('button', { name: /Schema/ });
    expect(header.getAttribute('aria-sort')).toBeNull();
    await userEvent.setup().click(header);
    expect(onSortChange).toHaveBeenCalledWith('blueprint_version');
  });
  it('shows the localized Sample badge and hides selection by default', () => {
    renderTable([item]);
    expect(screen.getByText('Sample')).toBeTruthy();
    expect(screen.queryByRole('checkbox')).toBeNull();
  });

  it('only offers deletion with permission and confirms before deleting', async () => {
    const user = userEvent.setup();
    const { unmount } = renderTable([item]);
    await user.click(
      screen.getByRole('button', { name: `Entity actions for ${item.id}` }),
    );
    expect(
      screen.queryByRole('menuitem', { name: 'Delete entity' }),
    ).toBeNull();
    unmount();

    vi.mocked(deleteEntity).mockResolvedValue(undefined);
    renderTable([item], true);
    await user.click(
      screen.getByRole('button', { name: `Entity actions for ${item.id}` }),
    );
    await user.click(screen.getByRole('menuitem', { name: 'Delete entity' }));
    expect(deleteEntity).not.toHaveBeenCalled();
    await user.click(screen.getByRole('button', { name: 'Cancel' }));
    expect(deleteEntity).not.toHaveBeenCalled();
    await user.click(
      screen.getByRole('button', { name: `Entity actions for ${item.id}` }),
    );
    await user.click(screen.getByRole('menuitem', { name: 'Delete entity' }));
    await user.click(
      within(screen.getByRole('dialog')).getByRole('button', {
        name: 'Delete entity',
      }),
    );
    await waitFor(() => expect(deleteEntity).toHaveBeenCalledWith(item.id));
  });

  it('keeps the confirmation open with an error when deletion fails', async () => {
    const user = userEvent.setup();
    vi.mocked(deleteEntity).mockRejectedValueOnce(
      new Error('Cannot delete entity'),
    );
    renderTable([item], true);
    await user.click(
      screen.getByRole('button', { name: `Entity actions for ${item.id}` }),
    );
    await user.click(screen.getByRole('menuitem', { name: 'Delete entity' }));
    await user.click(
      within(screen.getByRole('dialog')).getByRole('button', {
        name: 'Delete entity',
      }),
    );
    expect(await screen.findByText('Cannot delete entity')).toBeTruthy();
    expect(screen.getByRole('dialog')).toBeTruthy();
  });

  it('selects individual and loaded rows, clears and exits selection mode', async () => {
    outletMount.mockClear();
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
    expect(outletMount).toHaveBeenCalledWith({
      outlet: 'explorer_bulk_action',
      context: {
        context_version: 1,
        blueprint_id: blueprint.blueprint.id,
        blueprint_version: blueprint.blueprint.version,
        entity_ids: [item.id],
      },
      runtimeScope: {
        blueprintId: blueprint.blueprint.id,
        blueprintVersion: blueprint.blueprint.version,
      },
    });
    await user.click(selectAll);
    expect(selectAll.checked).toBe(true);
    expect(screen.getByText('2 selected')).toBeTruthy();
    await user.click(screen.getByRole('button', { name: 'Clear selection' }));
    expect(screen.getByText('0 selected')).toBeTruthy();
    expect(
      screen.getByRole('button', { name: 'Actions' }).hasAttribute('disabled'),
    ).toBe(true);
    expect(selectAll.checked).toBe(false);
    outletMount.mockClear();
    await user.click(screen.getByRole('button', { name: 'Exit selection' }));
    expect(outletMount).not.toHaveBeenCalledWith(
      expect.objectContaining({ outlet: 'explorer_bulk_action' }),
    );
    expect(screen.queryByRole('checkbox')).toBeNull();
    await user.click(screen.getByRole('button', { name: 'Select entities' }));
    expect(screen.getByText('0 selected')).toBeTruthy();
  });

  it('keeps selection across searches and lets it be reviewed', async () => {
    const user = userEvent.setup();
    const { search } = renderTable();
    await user.click(screen.getByRole('button', { name: 'Select entities' }));
    await user.click(
      screen.getByRole('checkbox', { name: 'Select Sample product' }),
    );
    search([secondItem], 'second-search');
    expect(screen.getByText('1 selected')).toBeTruthy();
    const selectAll = screen.getByRole('checkbox', {
      name: 'Select loaded entities (up to 50)',
    }) as HTMLInputElement;
    expect(selectAll.checked).toBe(false);
    expect(selectAll.getAttribute('data-indeterminate')).toBe('false');
    await user.click(
      screen.getByRole('checkbox', { name: 'Select Second product' }),
    );
    expect(screen.getByText('2 selected')).toBeTruthy();

    await user.click(screen.getByRole('button', { name: '2 selected' }));
    const review = screen.getByRole('dialog', { name: 'Selected entities' });
    expect(within(review).getByText('Sample product')).toBeTruthy();
    await user.click(
      within(review).getByRole('button', {
        name: 'Remove Sample product from selection',
      }),
    );
    expect(screen.getByText('1 selected')).toBeTruthy();
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
      screen.getByText(
        'Limit reached: up to 50 loaded entities can be selected',
      ),
    ).toBeTruthy();
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
    await user.click(screen.getByRole('button', { name: 'Actions' }));
    await user.click(
      screen.getByRole('menuitem', { name: 'Send to agent conversation' }),
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
    await user.click(screen.getByRole('button', { name: 'Actions' }));
    await user.click(
      screen.getByRole('menuitem', { name: 'Send to agent conversation' }),
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
