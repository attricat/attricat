// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import type { ComponentProps } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { ToastProvider } from '../../components/ToastProvider';
import type { BlueprintWithAttributes, RecordItem } from '../records/api';
import { extensionQueryKeys } from '../extensions/queryKeys';
import { ExplorerResultsTable } from './ExplorerResultsTable';
import { useExplorerSelection } from './useExplorerSelection';

const navigate = vi.fn();
const virtualWindow = vi.hoisted(() => ({ start: 0, size: Infinity }));
beforeEach(() => {
  virtualWindow.start = 0;
  virtualWindow.size = Infinity;
});
const access = vi.hoisted(() => ({ delete: false }));

vi.mock('../auth/useCan', () => ({
  useCan: () => true,
  useCanOn: (capability: string) =>
    capability === 'records_delete'
      ? access.delete
      : capability !== 'records_publish',
}));

vi.mock('@tanstack/react-router', () => ({
  createLink:
    () =>
    ({ children }: { children: React.ReactNode }) => <span>{children}</span>,
  Link: ({ children }: { children: React.ReactNode }) => (
    <span>{children}</span>
  ),
  useNavigate: () => navigate,
}));
vi.mock('@tanstack/react-virtual', () => ({
  useVirtualizer: ({ count }: { count: number }) => ({
    getTotalSize: () => count * 53,
    getVirtualItems: () =>
      Array.from(
        { length: Math.min(virtualWindow.size, count - virtualWindow.start) },
        (_, offset) => {
          const index = virtualWindow.start + offset;
          return {
            end: (index + 1) * 53,
            index,
            key: index,
            size: 53,
            start: index * 53,
          };
        },
      ),
    measureElement: vi.fn(),
  }),
}));
vi.mock('../records/api', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../records/api')>()),
  deleteRecord: vi.fn(),
  getRecordsPublications: vi.fn((recordIds: string[]) =>
    Promise.resolve({
      items: recordIds.map((record_id) => ({ record_id, publications: [] })),
    }),
  ),
}));
vi.mock('../extensions/api', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../extensions/api')>()),
  getExtensionRuntime: vi.fn().mockResolvedValue({ contributions: [] }),
}));
const { outletMount, popoverMount } = vi.hoisted(() => ({
  outletMount: vi.fn(),
  popoverMount: vi.fn(),
}));
vi.mock('../extensions/ExtensionActions', () => ({
  ExtensionActionsMenuButton: (props: unknown) => {
    popoverMount(props);
    return null;
  },
  ExtensionActionMenuItems: (props: unknown) => {
    popoverMount(props);
    return null;
  },
}));
vi.mock('../extensions/ExtensionOutlet', () => ({
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
import { deleteRecord, getRecordsPublications } from '../records/api';

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

const item: RecordItem = {
  blueprint_version: 1,
  display: { default: 'Sample product' },
  id: '123e4567-e89b-12d3-a456-426614174001',
  is_sample: true,
  system_tags: [],
  match_explanations: [],
  preview: {},
  related: {},
  schema_outdated: false,
  table_values: {},
};
const secondItem: RecordItem = {
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
  onSaveSelectionAsSearch = vi.fn(),
  publicationContextId?: string,
) => {
  access.delete = canDelete;
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false, staleTime: Infinity } },
  });
  queryClient.setQueryData(extensionQueryKeys.runtime(), {
    contributions: [],
  });
  const tree = (tableItems: RecordItem[], key: string) => (
    <QueryClientProvider client={queryClient}>
      <ToastProvider>
        <TableWithSelection
          tableKey={key}
          blueprint={blueprint}
          hasNextPage={false}
          isFetching={false}
          isFetchingNextPage={false}
          items={tableItems}
          onLoadMore={vi.fn()}
          onSaveSelectionAsSearch={onSaveSelectionAsSearch}
          onSortChange={onSortChange}
          sort={sort}
          contextCodes={['default']}
          publicationContextCode="default"
          publicationContextId={publicationContextId}
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
    queryClient,
    search: (nextItems: RecordItem[], key: string) =>
      view.rerender(tree(nextItems, key)),
  };
};

describe('ExplorerResultsTable', () => {
  it('bounds publication requests to virtual rows and reuses fresh results when scrolling back', async () => {
    vi.mocked(getRecordsPublications).mockClear();
    const requestedIds = () =>
      vi.mocked(getRecordsPublications).mock.calls.flatMap(([ids]) => ids);
    virtualWindow.size = 5;
    const items = Array.from({ length: 1000 }, (_, index) => ({
      ...item,
      id: `123e4567-e89b-12d3-a456-${String(index).padStart(12, '0')}`,
    }));
    const view = renderTable(
      items,
      false,
      vi.fn(),
      undefined,
      true,
      false,
      'initial',
      vi.fn(),
      'context',
    );
    // The visible rows share one lookup.
    await waitFor(() =>
      expect(getRecordsPublications).toHaveBeenCalledTimes(1),
    );
    expect(requestedIds()).toEqual(items.slice(0, 5).map(({ id }) => id));
    await waitFor(() => expect(view.queryClient.isFetching()).toBe(0));
    await userEvent.setup().click(
      screen.getByRole('button', {
        name: `Record actions for ${items[0].id}`,
      }),
    );
    virtualWindow.start = 200;
    view.search(items, 'initial');
    await waitFor(() =>
      expect(getRecordsPublications).toHaveBeenCalledTimes(2),
    );
    expect(requestedIds()).toHaveLength(10);
    await waitFor(() => expect(view.queryClient.isFetching()).toBe(0));
    expect(screen.getByRole('menu')).toBeTruthy();
    virtualWindow.start = 0;
    view.search(items, 'initial');
    await waitFor(() =>
      expect(
        screen.getAllByRole('row', { hidden: true }).length,
      ).toBeGreaterThan(1),
    );
    expect(getRecordsPublications).toHaveBeenCalledTimes(2);
  });

  it('offers revision-scoped Explorer actions only for single-version results', () => {
    popoverMount.mockClear();
    renderTable([item], false, vi.fn(), undefined, true, false);
    expect(popoverMount).not.toHaveBeenCalled();
    renderTable([item]);
    // One toolbar button offers the Explorer's revision-wide actions.
    expect(popoverMount).toHaveBeenCalledWith({
      outlet: 'explorer_action',
      label: 'Extension actions',
      runtimeScope: {
        blueprintId: blueprint.blueprint.id,
        blueprintVersion: blueprint.blueprint.version,
      },
      // A revision-wide action names no rows.
      target: expect.objectContaining({
        blueprintId: blueprint.blueprint.id,
        recordIds: [],
      }),
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
      screen.getByRole('button', { name: `Record actions for ${item.id}` }),
    );
    expect(
      screen.queryByRole('menuitem', { name: 'Delete record' }),
    ).toBeNull();
    unmount();

    vi.mocked(deleteRecord).mockResolvedValue(undefined);
    renderTable([item], true);
    await user.click(
      screen.getByRole('button', { name: `Record actions for ${item.id}` }),
    );
    await user.click(screen.getByRole('menuitem', { name: 'Delete record' }));
    expect(deleteRecord).not.toHaveBeenCalled();
    await user.click(screen.getByRole('button', { name: 'Cancel' }));
    expect(deleteRecord).not.toHaveBeenCalled();
    await user.click(
      screen.getByRole('button', { name: `Record actions for ${item.id}` }),
    );
    await user.click(screen.getByRole('menuitem', { name: 'Delete record' }));
    await user.click(
      within(screen.getByRole('dialog')).getByRole('button', {
        name: 'Delete record',
      }),
    );
    await waitFor(() => expect(deleteRecord).toHaveBeenCalledWith(item.id));
  });

  it('keeps the confirmation open with an error when deletion fails', async () => {
    const user = userEvent.setup();
    vi.mocked(deleteRecord).mockRejectedValueOnce(
      new Error('Cannot delete record'),
    );
    renderTable([item], true);
    await user.click(
      screen.getByRole('button', { name: `Record actions for ${item.id}` }),
    );
    await user.click(screen.getByRole('menuitem', { name: 'Delete record' }));
    await user.click(
      within(screen.getByRole('dialog')).getByRole('button', {
        name: 'Delete record',
      }),
    );
    expect(await screen.findByText('Cannot delete record')).toBeTruthy();
    expect(screen.getByRole('dialog')).toBeTruthy();
  });

  it('selects individual and loaded rows, clears and exits selection mode', async () => {
    const user = userEvent.setup();
    renderTable();
    await user.click(screen.getByRole('button', { name: 'Select records' }));
    const selectAll = screen.getByRole('checkbox', {
      name: 'Select loaded records (up to 50)',
    }) as HTMLInputElement;
    const first = screen.getByRole('checkbox', {
      name: 'Select Sample product',
    });
    await user.click(first);
    expect((first as HTMLInputElement).checked).toBe(true);
    expect(selectAll.getAttribute('data-indeterminate')).toBe('true');
    expect(screen.getByText('1 selected')).toBeTruthy();
    // The selection's actions menu offers its bulk actions, as a row's
    // menu offers the row's.
    popoverMount.mockClear();
    await user.click(screen.getByRole('button', { name: 'Actions' }));
    expect(popoverMount).toHaveBeenCalledWith({
      outlet: 'explorer_bulk_action',
      runtimeScope: {
        blueprintId: blueprint.blueprint.id,
        blueprintVersion: blueprint.blueprint.version,
      },
      target: expect.objectContaining({
        blueprintId: blueprint.blueprint.id,
        recordIds: [item.id],
      }),
    });
    await user.keyboard('{Escape}');
    await user.click(selectAll);
    expect(selectAll.checked).toBe(true);
    expect(screen.getByText('2 selected')).toBeTruthy();
    await user.click(screen.getByRole('button', { name: 'Clear selection' }));
    expect(screen.getByText('0 selected')).toBeTruthy();
    expect(
      screen.getByRole('button', { name: 'Actions' }).hasAttribute('disabled'),
    ).toBe(true);
    expect(selectAll.checked).toBe(false);
    await user.click(screen.getByRole('button', { name: 'Exit selection' }));
    expect(screen.queryByRole('checkbox')).toBeNull();
    await user.click(screen.getByRole('button', { name: 'Select records' }));
    expect(screen.getByText('0 selected')).toBeTruthy();
  });

  it('keeps selection across searches and lets it be reviewed', async () => {
    const user = userEvent.setup();
    const { search } = renderTable();
    await user.click(screen.getByRole('button', { name: 'Select records' }));
    await user.click(
      screen.getByRole('checkbox', { name: 'Select Sample product' }),
    );
    search([secondItem], 'second-search');
    expect(screen.getByText('1 selected')).toBeTruthy();
    const selectAll = screen.getByRole('checkbox', {
      name: 'Select loaded records (up to 50)',
    }) as HTMLInputElement;
    expect(selectAll.checked).toBe(false);
    expect(selectAll.getAttribute('data-indeterminate')).toBe('false');
    await user.click(
      screen.getByRole('checkbox', { name: 'Select Second product' }),
    );
    expect(screen.getByText('2 selected')).toBeTruthy();

    await user.click(screen.getByRole('button', { name: '2 selected' }));
    const review = screen.getByRole('dialog', { name: 'Selected records' });
    expect(within(review).getByText('Sample product')).toBeTruthy();
    await user.click(
      within(review).getByRole('button', {
        name: 'Remove Sample product from selection',
      }),
    );
    expect(screen.getByText('1 selected')).toBeTruthy();
  });

  it('limits bulk selection to 50 loaded records', async () => {
    const user = userEvent.setup();
    const items = Array.from({ length: 51 }, (_, index) => ({
      ...item,
      id: `123e4567-e89b-12d3-a456-${String(index).padStart(12, '0')}`,
      display: { default: `Product ${index}` },
    }));
    renderTable(items);
    await user.click(screen.getByRole('button', { name: 'Select records' }));
    await user.click(
      screen.getByRole('checkbox', {
        name: 'Select loaded records (up to 50)',
      }),
    );
    expect(screen.getByText('50 selected')).toBeTruthy();
    expect(
      screen.getByText(
        'Limit reached: up to 50 loaded records can be selected',
      ),
    ).toBeTruthy();
    expect(
      screen
        .getByRole('checkbox', { name: 'Select Product 50' })
        .hasAttribute('disabled'),
    ).toBe(true);
  });

  it('sends only selected record references to a new conversation', async () => {
    const user = userEvent.setup();
    vi.mocked(createConversation).mockResolvedValue({
      id: '123e4567-e89b-12d3-a456-426614174003',
      title: 'Review',
    });
    vi.mocked(sendMessage).mockResolvedValue({} as never);
    renderTable();
    await user.click(screen.getByRole('button', { name: 'Select records' }));
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
    expect(createConversation).toHaveBeenCalledWith('Review 1 Product records');
    expect(sendMessage).toHaveBeenCalledWith(
      '123e4567-e89b-12d3-a456-426614174003',
      expect.stringContaining(`record_id: ${item.id}`),
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

  it('creates a saved search from the selected records', async () => {
    const user = userEvent.setup();
    const onSaveSelectionAsSearch = vi.fn();
    renderTable(
      undefined,
      false,
      undefined,
      undefined,
      true,
      true,
      'initial',
      onSaveSelectionAsSearch,
    );
    await user.click(screen.getByRole('button', { name: 'Select records' }));
    await user.click(
      screen.getByRole('checkbox', { name: 'Select Second product' }),
    );
    await user.click(screen.getByRole('button', { name: 'Actions' }));
    await user.click(
      screen.getByRole('menuitem', { name: 'Create saved search' }),
    );
    expect(onSaveSelectionAsSearch).toHaveBeenCalledWith([secondItem]);
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
    await user.click(screen.getByRole('button', { name: 'Select records' }));
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
