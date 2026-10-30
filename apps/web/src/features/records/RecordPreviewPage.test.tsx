// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { act, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { ToastProvider } from '../../components/ToastProvider';
import { listContexts } from '../contexts/api';
import { RecordPreviewPage } from './RecordPreviewPage';

const {
  actionsRender,
  agentDrawerRender,
  fieldNotesRender,
  outletRender,
  frameRender,
  navigate,
} = vi.hoisted(() => ({
  navigate: vi.fn(),
  frameRender: vi.fn(),
  agentDrawerRender: vi.fn(),
  outletRender: vi.fn(),
  fieldNotesRender: vi.fn(),
  actionsRender: vi.fn(),
}));

vi.mock('@tanstack/react-router', () => ({
  createLink: <T,>(component: T) => component,
  Link: ({ children }: { children: React.ReactNode }) => (
    <span>{children}</span>
  ),
  useNavigate: () => navigate,
}));

vi.mock('./api', async (importOriginal) => ({
  ...(await importOriginal<typeof import('./api')>()),
  duplicateRecord: vi.fn(),
  deleteRecord: vi.fn(),
  getBlueprintRevision: vi.fn(),
  getCurrentBlueprint: vi.fn(),
  getRecordApprovals: vi.fn(),
  getRecordForm: vi.fn(),
  getRecordLabels: vi.fn(),
  getRecordPublications: vi.fn(),
  getRecordRetentionHolds: vi.fn(),
  getResolvedRecordPreview: vi.fn(),
  publishRecord: vi.fn(),
  publishRecordAllChannels: vi.fn(),
  unpublishRecord: vi.fn(),
}));

vi.mock('../extensions/api', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../extensions/api')>()),
  getExtensionRuntime: vi.fn(),
}));
vi.mock('../extensions/ExtensionActions', () => ({
  ExtensionActionButtons: (props: unknown) => {
    actionsRender(props);
    return null;
  },
  ExtensionActionMenuItems: (props: unknown) => {
    actionsRender(props);
    return null;
  },
}));
vi.mock('../extensions/ExtensionFrame', () => ({
  ExtensionFrame: (props: { contribution: { id: string } }) => {
    frameRender(props);
    return <span>{`frame:${props.contribution.id}`}</span>;
  },
}));
vi.mock('../extensions/ExtensionOutlet', () => ({
  ExtensionOutlet: (props: unknown) => {
    outletRender(props);
    return null;
  },
}));

vi.mock('../rules/api', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../rules/api')>()),
  getUnresolvedRecordFindingCount: vi.fn(),
  listRecordRuleRuns: vi.fn(),
  listRecordRules: vi.fn(),
  listUnresolvedRecordFindings: vi.fn(),
}));

vi.mock('../extension-exports/api', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../extension-exports/api')>()),
  recordExportStatus: vi.fn(async () => ({ statuses: [], destinations: [] })),
}));

vi.mock('../record-comments/RecordCommentsPanel', () => ({
  RecordCommentsPanel: () => <section aria-label="Comments" />,
}));

vi.mock('./components/RecordAgentDrawer', () => ({
  RecordAgentDrawer: (props: unknown) => {
    agentDrawerRender(props);
    return null;
  },
}));

vi.mock('../views/components/RecordView', () => ({
  RecordView: () => (
    <div data-field-code="title">
      <input aria-label="Title value" />
    </div>
  ),
}));
vi.mock('../extensions/fieldNotes', () => ({
  FieldNotesProvider: ({
    children,
    ...props
  }: {
    children: React.ReactNode;
  } & Record<string, unknown>) => {
    fieldNotesRender(props);
    return children;
  },
}));

vi.mock('../auth/api', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../auth/api')>()),
  currentSession: vi.fn().mockResolvedValue(null),
}));

vi.mock('../contexts/api', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../contexts/api')>()),
  listContexts: vi.fn(),
}));

const renderPage = (relationshipPickerToken?: string, tab?: string) => {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });

  return render(
    <QueryClientProvider client={queryClient}>
      <ToastProvider>
        <RecordPreviewPage
          recordId="00000000-0000-4000-8000-000000000001"
          relationshipPickerToken={relationshipPickerToken}
          tab={tab}
        />
      </ToastProvider>
    </QueryClientProvider>,
  );
};

describe('RecordPreviewPage', () => {
  beforeEach(async () => {
    const { getExtensionRuntime } = await import('../extensions/api');
    vi.mocked(getExtensionRuntime).mockReset();
    vi.mocked(getExtensionRuntime).mockResolvedValue([]);
    const { currentSession } = await import('../auth/api');
    vi.mocked(currentSession).mockReset();
    vi.mocked(currentSession).mockResolvedValue(null);
    const api = await import('./api');
    vi.mocked(api.getRecordPublications).mockResolvedValue([]);
    vi.mocked(api.getRecordLabels).mockReset();
    vi.mocked(api.getRecordLabels).mockResolvedValue({ items: [] });
    vi.mocked(api.getRecordForm).mockReset();
    vi.mocked(api.getRecordForm).mockRejectedValue(new Error('not loaded'));
    const emptyPage = { items: [], has_more: false };
    vi.mocked(api.getRecordApprovals).mockResolvedValue(emptyPage);
    vi.mocked(api.getRecordRetentionHolds).mockResolvedValue(emptyPage);
    // Every reader of a record sees its data quality.
    const rules = await import('../rules/api');
    vi.mocked(rules.listRecordRules).mockResolvedValue([]);
    vi.mocked(rules.listRecordRuleRuns).mockResolvedValue([]);
    vi.mocked(rules.listUnresolvedRecordFindings).mockResolvedValue({
      items: [],
    });
    vi.mocked(rules.getUnresolvedRecordFindingCount).mockResolvedValue({
      count: 0,
    });
  });
  it('offers confirmed deletion only when permitted and navigates away afterwards', async () => {
    const { getExtensionRuntime } = await import('../extensions/api');
    outletRender.mockClear();
    vi.mocked(getExtensionRuntime).mockResolvedValue([
      { outlet: 'record_attribute_panel', kind: 'panel' },
      { outlet: 'file_panel', kind: 'panel' },
    ] as never);
    const { currentSession } = await import('../auth/api');
    const api = await import('./api');
    const user = userEvent.setup();
    vi.mocked(listContexts).mockResolvedValue([
      {
        id: '33333333-3333-4333-8333-333333333333',
        code: 'default',
        data: {},
        parent_id: null,
      },
    ]);
    vi.mocked(api.getResolvedRecordPreview).mockResolvedValue({
      record: {
        id: '00000000-0000-4000-8000-000000000001',
        blueprint_id: '22222222-2222-4222-8222-222222222222',
        blueprint_version: 1,
      },
      values: {},
      reusable_values: {},
    } as never);
    vi.mocked(api.getBlueprintRevision).mockResolvedValue({
      blueprint: {
        id: '22222222-2222-4222-8222-222222222222',
        name: 'Product',
        code: 'product',
        version: 1,
        views: {},
      },
      attributes: [],
    } as never);
    vi.mocked(api.getCurrentBlueprint).mockResolvedValue({
      blueprint: { version: 1 },
    } as never);
    vi.mocked(currentSession).mockResolvedValueOnce({
      capabilities: { records_delete: false },
    } as never);
    const { unmount } = renderPage();
    // The eyebrow links the blueprint once the record has loaded.
    await screen.findByText('See all');
    // Field notes are asked for once, for this record in its context.
    await waitFor(() =>
      expect(fieldNotesRender).toHaveBeenCalledWith({
        conditions: expect.objectContaining({ blueprintCode: 'product' }),
        contextId: '33333333-3333-4333-8333-333333333333',
        recordId: '00000000-0000-4000-8000-000000000001',
        runtimeScope: {
          blueprintId: '22222222-2222-4222-8222-222222222222',
          blueprintVersion: 1,
        },
      }),
    );
    await user.click(screen.getByRole('button', { name: 'Actions' }));
    expect(
      within(screen.getByRole('menu')).queryByRole('menuitem', {
        name: 'Delete record',
      }),
    ).toBeNull();
    unmount();

    // Panels that mount later refetch the session, so keep the grant.
    vi.mocked(currentSession).mockResolvedValue({
      capabilities: { records_delete: true },
    } as never);
    vi.mocked(api.deleteRecord).mockResolvedValue(undefined);
    renderPage();
    await user.click(screen.getByRole('button', { name: 'Actions' }));
    await user.click(
      await within(screen.getByRole('menu')).findByRole('menuitem', {
        name: 'Delete record',
      }),
    );
    expect(api.deleteRecord).not.toHaveBeenCalled();
    await user.click(
      within(screen.getByRole('dialog')).getByRole('button', {
        name: 'Delete record',
      }),
    );
    await waitFor(() =>
      expect(api.deleteRecord).toHaveBeenCalledWith(
        '00000000-0000-4000-8000-000000000001',
      ),
    );
    await waitFor(() => expect(navigate).toHaveBeenCalledWith({ to: '/' }));
  });

  it('duplicates only after confirmation and opens the copy', async () => {
    const api = await import('./api');
    const user = userEvent.setup();
    vi.mocked(listContexts).mockResolvedValue([]);
    vi.mocked(api.duplicateRecord).mockResolvedValue({
      id: '99999999-9999-4999-8999-999999999999',
    } as never);
    vi.mocked(api.getRecordForm).mockResolvedValue({
      can_write: true,
      record: { system_tags: [], system_metadata: {} },
      blueprint: {
        blueprint: { id: '22222222-2222-4222-8222-222222222222', version: 1 },
        attributes: [],
      },
    } as never);
    navigate.mockClear();
    renderPage();

    await user.click(screen.getByRole('button', { name: 'Actions' }));
    await user.click(
      within(screen.getByRole('menu')).getByRole('menuitem', {
        name: 'Duplicate record',
      }),
    );
    const dialog = screen.getByRole('dialog');
    expect(
      within(dialog).getByText(/Create a copy of this record/),
    ).toBeTruthy();
    expect(api.duplicateRecord).not.toHaveBeenCalled();

    await user.click(
      within(dialog).getByRole('button', { name: 'Duplicate record' }),
    );
    await waitFor(() =>
      expect(navigate).toHaveBeenCalledWith({
        params: { recordId: '99999999-9999-4999-8999-999999999999' },
        to: '/records/$recordId',
      }),
    );
    expect(api.duplicateRecord).toHaveBeenCalledWith(
      '00000000-0000-4000-8000-000000000001',
    );
  });

  it('returns a picker selection to its opener and closes the preview', () => {
    const postMessage = vi.fn();
    const close = vi.spyOn(window, 'close').mockImplementation(() => undefined);
    Object.defineProperty(window, 'opener', {
      configurable: true,
      value: { postMessage },
    });
    vi.mocked(listContexts).mockResolvedValue([]);

    renderPage('picker-token');
    screen
      .getByRole('button', { name: 'Select this record and close' })
      .click();

    expect(postMessage).toHaveBeenCalledWith(
      {
        type: 'attricat.relationship-picker.select',
        token: 'picker-token',
        recordId: '00000000-0000-4000-8000-000000000001',
      },
      window.location.origin,
    );
    expect(close).toHaveBeenCalled();

    close.mockRestore();
    Object.defineProperty(window, 'opener', {
      configurable: true,
      value: null,
    });
  });

  it('scopes every blueprint-owned extension surface to the pinned revision', async () => {
    const api = await import('./api');
    const { getExtensionRuntime } = await import('../extensions/api');
    vi.mocked(getExtensionRuntime).mockResolvedValue([
      { outlet: 'record_attribute_decoration', kind: 'embedded' },
    ] as never);
    const { currentSession } = await import('../auth/api');
    vi.mocked(currentSession).mockResolvedValue({
      capabilities: { agents_run: true },
    } as never);
    const blueprintId = '22222222-2222-4222-8222-222222222222';
    vi.mocked(listContexts).mockResolvedValue([
      {
        id: '33333333-3333-4333-8333-333333333333',
        code: 'default',
        data: {},
        parent_id: null,
      },
    ]);
    vi.mocked(api.getResolvedRecordPreview).mockResolvedValue({
      record: {
        id: '00000000-0000-4000-8000-000000000001',
        blueprint_id: blueprintId,
        blueprint_version: 7,
        is_sample: true,
      },
      values: {},
      reusable_values: {},
    } as never);
    const blueprint = {
      blueprint: {
        id: blueprintId,
        code: 'product',
        name: 'Product',
        version: 7,
        status: 'published',
        views: { detail: { type: 'stack', children: [] } },
      },
      attributes: [{ id: 'attribute-id', code: 'title', value_type: 'string' }],
      table_path_attributes: [],
    };
    vi.mocked(api.getBlueprintRevision).mockResolvedValue(blueprint as never);
    vi.mocked(api.getCurrentBlueprint).mockResolvedValue(blueprint as never);

    renderPage();

    expect(await screen.findByText('Sample')).toBeTruthy();
    const runtimeScope = { blueprintId, blueprintVersion: 7 };
    // Record actions are host-rendered buttons acting on this record.
    await waitFor(() =>
      expect(actionsRender).toHaveBeenCalledWith(
        expect.objectContaining({
          outlet: 'record_action',
          runtimeScope,
          target: expect.objectContaining({
            recordIds: ['00000000-0000-4000-8000-000000000001'],
          }),
        }),
      ),
    );
    expect(fieldNotesRender).toHaveBeenCalledWith(
      expect.objectContaining({ runtimeScope }),
    );
    // Header actions are items of the record's actions menu.
    expect(actionsRender).toHaveBeenCalledWith(
      expect.objectContaining({ outlet: 'record_header_action', runtimeScope }),
    );
    expect(outletRender).not.toHaveBeenCalledWith(
      expect.objectContaining({ outlet: 'record_header_action' }),
    );
    await userEvent
      .setup()
      .click(screen.getByRole('button', { name: 'Ask about this record' }));
    expect(agentDrawerRender).toHaveBeenCalledWith(
      expect.objectContaining({
        recordId: '00000000-0000-4000-8000-000000000001',
        contextId: '33333333-3333-4333-8333-333333333333',
        open: true,
      }),
    );
  });

  it('adds extension tabs for the record, mounting each frame once opened', async () => {
    const user = userEvent.setup();
    const api = await import('./api');
    const { getExtensionRuntime } = await import('../extensions/api');
    const tab = (extension: string, id: string, title: string) => ({
      contribution_key: `${extension}:${id}`,
      extension_id: extension,
      id,
      kind: 'tab',
      outlet: 'record_tab',
      release_id: '44444444-4444-4444-8444-444444444444',
      title,
      translations: {},
      icon: null,
      when: null,
    });
    vi.mocked(getExtensionRuntime).mockResolvedValue([
      tab('acme.inventory', 'stock', 'Stock'),
      {
        ...tab('acme.inventory', 'variants', 'Variants'),
        when: { blueprints: ['variant'] },
      },
      {
        ...tab('acme.pricing', 'prices', 'Prices'),
        when: { blueprints: ['product'] },
      },
      tab('acme.reviews', 'reviews', 'Reviews'),
      tab('acme.shipping', 'rates', 'Rates'),
    ] as never);
    const blueprintId = '22222222-2222-4222-8222-222222222222';
    const contextId = '33333333-3333-4333-8333-333333333333';
    vi.mocked(listContexts).mockResolvedValue([
      { id: contextId, code: 'default', data: {}, parent_id: null },
    ]);
    vi.mocked(api.getResolvedRecordPreview).mockResolvedValue({
      record: {
        id: '00000000-0000-4000-8000-000000000001',
        blueprint_id: blueprintId,
        blueprint_version: 7,
      },
      values: {},
      reusable_values: {},
    } as never);
    const blueprint = {
      blueprint: {
        id: blueprintId,
        code: 'product',
        name: 'Product',
        version: 7,
        status: 'published',
        views: { detail: { type: 'stack', children: [] } },
      },
      attributes: [],
      table_path_attributes: [],
    };
    vi.mocked(api.getBlueprintRevision).mockResolvedValue(blueprint as never);
    vi.mocked(api.getCurrentBlueprint).mockResolvedValue(blueprint as never);
    frameRender.mockClear();
    navigate.mockClear();

    renderPage();

    await screen.findByRole('tab', { name: 'Stock' });
    // A tab for another blueprint is left out; the fourth waits in More.
    expect(screen.queryByRole('tab', { name: 'Variants' })).toBeNull();
    expect(screen.getByRole('tab', { name: 'Prices' })).toBeTruthy();
    expect(screen.getByRole('tab', { name: 'Reviews' })).toBeTruthy();
    expect(screen.queryByRole('tab', { name: 'Rates' })).toBeNull();
    expect(frameRender).not.toHaveBeenCalled();

    await user.click(screen.getByRole('tab', { name: 'Stock' }));
    expect(await screen.findByText('frame:stock')).toBeTruthy();
    expect(frameRender).toHaveBeenLastCalledWith(
      expect.objectContaining({
        context: {
          context_version: 1,
          record_id: '00000000-0000-4000-8000-000000000001',
          context_id: contextId,
          blueprint_id: blueprintId,
          blueprint_version: 7,
        },
      }),
    );
    // The open tab is kept in the address so a link can return to it.
    expect(navigate).toHaveBeenLastCalledWith(
      expect.objectContaining({ replace: true }),
    );
    expect(
      navigate.mock.lastCall?.[0].search({ relationshipPicker: 'x' }),
    ).toEqual({ relationshipPicker: 'x', tab: 'acme.inventory:stock' });

    await user.click(screen.getByRole('tab', { name: 'More' }));
    await user.click(await screen.findByRole('menuitem', { name: 'Rates' }));
    expect(await screen.findByText('frame:rates')).toBeTruthy();
    // The More tab stands in for the open overflow tab.
    expect(
      screen.getByRole('tab', { name: 'Rates' }).getAttribute('aria-selected'),
    ).toBe('true');
    // Opened frames stay mounted when another tab is shown.
    expect(screen.getByText('frame:stock')).toBeTruthy();
  });

  it('opens the extension tab a link names', async () => {
    const api = await import('./api');
    const { getExtensionRuntime } = await import('../extensions/api');
    vi.mocked(getExtensionRuntime).mockResolvedValue([
      {
        contribution_key: 'acme.inventory:stock',
        extension_id: 'acme.inventory',
        id: 'stock',
        kind: 'tab',
        outlet: 'record_tab',
        release_id: '44444444-4444-4444-8444-444444444444',
        title: 'Stock',
        translations: { pl: { title: 'Stany' } },
        icon: 'tag',
        when: null,
      },
    ] as never);
    vi.mocked(listContexts).mockResolvedValue([
      {
        id: '33333333-3333-4333-8333-333333333333',
        code: 'default',
        data: {},
        parent_id: null,
      },
    ]);
    vi.mocked(api.getResolvedRecordPreview).mockResolvedValue({
      record: {
        id: '00000000-0000-4000-8000-000000000001',
        blueprint_id: '22222222-2222-4222-8222-222222222222',
        blueprint_version: 7,
      },
      values: {},
      reusable_values: {},
    } as never);
    const blueprint = {
      blueprint: {
        id: '22222222-2222-4222-8222-222222222222',
        code: 'product',
        name: 'Product',
        version: 7,
        status: 'published',
        views: { detail: { type: 'stack', children: [] } },
      },
      attributes: [],
      table_path_attributes: [],
    };
    vi.mocked(api.getBlueprintRevision).mockResolvedValue(blueprint as never);
    vi.mocked(api.getCurrentBlueprint).mockResolvedValue(blueprint as never);

    renderPage(undefined, 'acme.inventory:stock');

    expect(await screen.findByText('frame:stock')).toBeTruthy();
    expect(
      screen.getByRole('tab', { name: 'Stock' }).getAttribute('aria-selected'),
    ).toBe('true');
  });

  it('shows the data, data quality, comments, record control and exports in tabs, one at a time', async () => {
    const user = userEvent.setup();
    const scrollIntoView = vi.fn();
    Element.prototype.scrollIntoView = scrollIntoView;
    const { currentSession } = await import('../auth/api');
    const api = await import('./api');
    const rules = await import('../rules/api');
    vi.mocked(currentSession).mockResolvedValue({
      capabilities: { rules_read: true },
    } as never);
    vi.mocked(rules.getUnresolvedRecordFindingCount).mockResolvedValue({
      count: 2,
    });
    vi.mocked(listContexts).mockResolvedValue([
      {
        id: '33333333-3333-4333-8333-333333333333',
        code: 'default',
        data: {},
        parent_id: null,
      },
    ]);
    vi.mocked(api.getResolvedRecordPreview).mockResolvedValue({
      record: {
        id: '00000000-0000-4000-8000-000000000001',
        blueprint_id: '22222222-2222-4222-8222-222222222222',
        blueprint_version: 1,
      },
      values: {},
      reusable_values: {},
    } as never);
    vi.mocked(api.getBlueprintRevision).mockResolvedValue({
      blueprint: {
        id: '22222222-2222-4222-8222-222222222222',
        name: 'Product',
        code: 'product',
        version: 1,
        views: {},
      },
      attributes: [],
    } as never);
    vi.mocked(api.getCurrentBlueprint).mockResolvedValue({
      blueprint: { version: 1 },
    } as never);
    vi.mocked(api.getRecordRetentionHolds).mockResolvedValue({
      items: [
        {
          id: '00000000-0000-4000-8000-000000000010',
          file_id: '00000000-0000-4000-8000-000000000011',
          source: 'explicit',
          attribute_code: null,
          status: null,
          reason: 'Legal hold',
          held_until: '2027-10-20T10:00:00Z',
          created_at: '2026-10-20T10:00:00Z',
          released_at: null,
          active: true,
        },
      ],
      has_more: false,
    } as never);
    renderPage();

    const tabs = within(await screen.findByRole('tablist')).getAllByRole('tab');
    expect(tabs.map((tab) => tab.textContent)).toEqual([
      'Data',
      'Data quality',
      'Comments',
      'Record control',
      'Tags and metadata',
      'Exports',
    ]);
    // The record's values come first.
    expect(screen.getByRole('tabpanel', { name: 'Data' })).toBeTruthy();
    // Hidden panels stay mounted but out of the accessibility tree.
    expect(screen.getAllByRole('tabpanel')).toHaveLength(1);

    await user.click(screen.getByRole('tab', { name: 'Data quality' }));
    expect(
      (await screen.findByRole('tabpanel', { name: 'Data quality' }))
        .textContent,
    ).toContain('No open findings.');

    await user.click(screen.getByRole('tab', { name: 'Record control' }));
    expect(
      within(
        screen.getByRole('tabpanel', { name: 'Record control' }),
      ).getByText('Retained: Legal hold'),
    ).toBeTruthy();
    expect(screen.queryByRole('tabpanel', { name: 'Data quality' })).toBeNull();

    await user.click(screen.getByRole('tab', { name: 'Comments' }));
    expect(
      within(screen.getByRole('tabpanel', { name: 'Comments' })).getByRole(
        'region',
        { name: 'Comments' },
      ),
    ).toBeTruthy();

    // The header's findings chip opens their tab and moves to it.
    await user.click(
      await screen.findByRole('button', { name: '2 data quality findings' }),
    );
    const dataQuality = screen.getByRole('tabpanel', { name: 'Data quality' });
    expect(
      screen
        .getByRole('tab', { name: 'Data quality' })
        .getAttribute('aria-selected'),
    ).toBe('true');
    await waitFor(() => expect(document.activeElement).toBe(dataQuality));
    expect(scrollIntoView).toHaveBeenCalled();
  });

  it('hides record control when the record has no approvals or holds', async () => {
    const api = await import('./api');
    vi.mocked(listContexts).mockResolvedValue([
      {
        id: '33333333-3333-4333-8333-333333333333',
        code: 'default',
        data: {},
        parent_id: null,
      },
    ]);
    vi.mocked(api.getResolvedRecordPreview).mockResolvedValue({
      record: {
        id: '00000000-0000-4000-8000-000000000001',
        blueprint_id: '22222222-2222-4222-8222-222222222222',
        blueprint_version: 1,
      },
      values: {},
      reusable_values: {},
    } as never);
    vi.mocked(api.getBlueprintRevision).mockResolvedValue({
      blueprint: {
        id: '22222222-2222-4222-8222-222222222222',
        name: 'Product',
        code: 'product',
        version: 1,
        views: {},
      },
      attributes: [],
    } as never);
    vi.mocked(api.getCurrentBlueprint).mockResolvedValue({
      blueprint: { version: 1 },
    } as never);
    renderPage();

    await screen.findByRole('tablist');
    // Both lists have answered empty.
    await waitFor(() => expect(api.getRecordRetentionHolds).toHaveBeenCalled());
    await act(async () => {});
    expect(
      within(screen.getByRole('tablist'))
        .getAllByRole('tab')
        .map((tab) => tab.textContent),
    ).not.toContain('Record control');
  });

  it('opens the data tab at the field a data quality finding is about', async () => {
    const user = userEvent.setup();
    const api = await import('./api');
    const rules = await import('../rules/api');
    vi.mocked(listContexts).mockResolvedValue([
      {
        id: '33333333-3333-4333-8333-333333333333',
        code: 'default',
        data: {},
        parent_id: null,
      },
    ]);
    vi.mocked(api.getResolvedRecordPreview).mockResolvedValue({
      record: {
        id: '00000000-0000-4000-8000-000000000001',
        blueprint_id: '22222222-2222-4222-8222-222222222222',
        blueprint_version: 1,
      },
      values: {},
      reusable_values: {},
    } as never);
    vi.mocked(api.getBlueprintRevision).mockResolvedValue({
      blueprint: {
        id: '22222222-2222-4222-8222-222222222222',
        name: 'Product',
        code: 'product',
        version: 1,
        views: {},
      },
      attributes: [{ code: 'title', name: 'Listing title' }],
    } as never);
    vi.mocked(api.getCurrentBlueprint).mockResolvedValue({
      blueprint: { version: 1 },
    } as never);
    vi.mocked(rules.listUnresolvedRecordFindings).mockResolvedValue({
      items: [
        {
          id: '00000000-0000-4000-8000-000000000004',
          rule_id: '00000000-0000-4000-8000-000000000003',
          rule_version: 1,
          record_id: '00000000-0000-4000-8000-000000000001',
          context_id: null,
          severity: 'error',
          message: "'title' is required",
          evidence: { attribute_code: 'title' },
          state: 'open',
          acknowledged_at: null,
          resolved_at: null,
          created_at: '2026-10-20T10:00:00Z',
          updated_at: '2026-10-20T10:00:00Z',
        },
      ],
    });
    renderPage();

    await user.click(await screen.findByRole('tab', { name: 'Data quality' }));
    await user.click(
      within(
        await screen.findByRole('tabpanel', { name: 'Data quality' }),
      ).getByRole('button', { name: 'Listing title' }),
    );

    expect(
      screen.getByRole('tab', { name: 'Data' }).getAttribute('aria-selected'),
    ).toBe('true');
    await waitFor(() =>
      expect(document.activeElement).toBe(
        screen.getByRole('textbox', { name: 'Title value' }),
      ),
    );
  });

  it('shows a retryable error instead of a blank preview when contexts fail to load', async () => {
    vi.mocked(listContexts).mockRejectedValue(
      new Error('contexts unavailable'),
    );

    renderPage();

    expect(
      await screen.findByText('Unable to load this information'),
    ).toBeTruthy();
    expect(screen.queryByLabelText('Context')).toBeNull();

    screen.getByRole('button', { name: 'Try again' }).click();

    await waitFor(() => expect(listContexts).toHaveBeenCalledTimes(2));
  });

  const loadRecord = async (views: Record<string, unknown>) => {
    const api = await import('./api');
    vi.mocked(listContexts).mockResolvedValue([
      {
        id: '33333333-3333-4333-8333-333333333333',
        code: 'default',
        data: {},
        parent_id: null,
      },
    ]);
    vi.mocked(api.getResolvedRecordPreview).mockResolvedValue({
      record: {
        id: '00000000-0000-4000-8000-000000000001',
        blueprint_id: '22222222-2222-4222-8222-222222222222',
        blueprint_version: 1,
      },
      values: { name: { value: 'Blue lamp' } },
      reusable_values: {},
    } as never);
    vi.mocked(api.getBlueprintRevision).mockResolvedValue({
      blueprint: {
        id: '22222222-2222-4222-8222-222222222222',
        name: 'Product',
        code: 'product',
        version: 1,
        views,
      },
      attributes: [{ code: 'name', value_type: 'string' }],
    } as never);
    vi.mocked(api.getCurrentBlueprint).mockResolvedValue({
      blueprint: { version: 1 },
    } as never);
    vi.mocked(api.getRecordLabels).mockResolvedValue({
      items: [
        {
          id: '00000000-0000-4000-8000-000000000001',
          blueprint_code: 'product',
          display: { default: 'Blue lamp · LMP-1' },
        },
      ],
    });
  };

  it('titles the browser tab with the page_title view', async () => {
    await loadRecord({
      page_title: { type: 'page_title', fields: ['name'] },
    });
    renderPage();

    await waitFor(() => expect(document.title).toBe('Blue lamp · Attricat'));
  });

  it('titles the browser tab with the dropdown label without a page_title view', async () => {
    await loadRecord({});
    renderPage();

    await waitFor(() =>
      expect(document.title).toBe('Blue lamp · LMP-1 · Attricat'),
    );
  });

  it('waits for the form instead of showing read-only fields first', async () => {
    await loadRecord({});
    const api = await import('./api');
    vi.mocked(api.getRecordForm).mockReturnValue(new Promise(() => {}));
    renderPage();

    expect(await screen.findByLabelText('Loading fields...')).toBeTruthy();
    expect(screen.queryByLabelText('Title value')).toBeNull();
  });

  it('shows the values read-only when the form cannot be loaded', async () => {
    await loadRecord({});
    renderPage();

    expect(await screen.findByLabelText('Title value')).toBeTruthy();
    expect(screen.queryByLabelText('Loading fields...')).toBeNull();
  });
});
