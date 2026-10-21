// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { ToastProvider } from '../../components/ToastProvider';
import { listContexts } from '../contexts/api';
import { RecordPreviewPage } from './RecordPreviewPage';

const {
  drawerRender,
  agentDrawerRender,
  outletRender,
  popoverOutletRender,
  navigate,
} = vi.hoisted(() => ({
  navigate: vi.fn(),
  drawerRender: vi.fn(),
  agentDrawerRender: vi.fn(),
  outletRender: vi.fn(),
  popoverOutletRender: vi.fn(),
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
vi.mock('../extensions/ExtensionOutlet', () => ({
  ExtensionOutlet: (props: unknown) => {
    outletRender(props);
    return null;
  },
  ExtensionPopoverOutlet: (props: unknown) => {
    popoverOutletRender(props);
    return null;
  },
}));

vi.mock('../rules/api', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../rules/api')>()),
  getUnresolvedRecordFindingCount: vi.fn(),
  listRecordRuleRuns: vi.fn(),
  listRules: vi.fn(),
  listUnresolvedRecordFindings: vi.fn(),
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

vi.mock('./components/RecordExtensionDrawer', () => ({
  RecordExtensionDrawer: (props: unknown) => {
    drawerRender(props);
    return null;
  },
}));

vi.mock('../views/components/RecordView', () => ({
  RecordView: ({
    renderAttributeDecoration,
    renderAttributePanel,
    renderFilePanel,
  }: {
    renderAttributeDecoration?: (attribute: {
      id: string;
      code: string;
    }) => React.ReactNode;
    renderAttributePanel?: (attribute: {
      id: string;
      code: string;
    }) => React.ReactNode;
    renderFilePanel?: (
      attribute: { id: string; code: string },
      fileId: string,
    ) => React.ReactNode;
  }) => (
    <>
      {renderAttributeDecoration?.({
        id: '44444444-4444-4444-8444-444444444444',
        code: 'title',
      })}
      {renderAttributePanel?.({
        id: '44444444-4444-4444-8444-444444444444',
        code: 'title',
      })}
      {renderFilePanel?.(
        { id: '44444444-4444-4444-8444-444444444444', code: 'file' },
        '55555555-5555-4555-8555-555555555555',
      )}
    </>
  ),
}));

vi.mock('../auth/api', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../auth/api')>()),
  currentSession: vi.fn().mockResolvedValue(null),
}));

vi.mock('../contexts/api', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../contexts/api')>()),
  listContexts: vi.fn(),
}));

const renderPage = (relationshipPickerToken?: string) => {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });

  return render(
    <QueryClientProvider client={queryClient}>
      <ToastProvider>
        <RecordPreviewPage
          recordId="00000000-0000-4000-8000-000000000001"
          relationshipPickerToken={relationshipPickerToken}
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
    vi.mocked(api.getRecordForm).mockReset();
    vi.mocked(api.getRecordForm).mockRejectedValue(new Error('not loaded'));
    const emptyPage = { items: [], has_more: false };
    vi.mocked(api.getRecordApprovals).mockResolvedValue(emptyPage);
    vi.mocked(api.getRecordRetentionHolds).mockResolvedValue(emptyPage);
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
    await waitFor(() =>
      expect(outletRender).toHaveBeenCalledWith({
        outlet: 'record_attribute_panel',
        context: {
          context_version: 1,
          record_id: '00000000-0000-4000-8000-000000000001',
          attribute_id: '44444444-4444-4444-8444-444444444444',
          blueprint_id: '22222222-2222-4222-8222-222222222222',
          blueprint_version: 1,
          context_id: '33333333-3333-4333-8333-333333333333',
        },
        runtimeScope: {
          blueprintId: '22222222-2222-4222-8222-222222222222',
          blueprintVersion: 1,
        },
      }),
    );
    expect(outletRender).toHaveBeenCalledWith({
      outlet: 'file_panel',
      context: {
        context_version: 1,
        file_id: '55555555-5555-4555-8555-555555555555',
        record_id: '00000000-0000-4000-8000-000000000001',
        attribute_id: '44444444-4444-4444-8444-444444444444',
        blueprint_id: '22222222-2222-4222-8222-222222222222',
        blueprint_version: 1,
      },
      runtimeScope: {
        blueprintId: '22222222-2222-4222-8222-222222222222',
        blueprintVersion: 1,
      },
    });
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
    await waitFor(() =>
      expect(outletRender).toHaveBeenCalledWith(
        expect.objectContaining({ outlet: 'record_action', runtimeScope }),
      ),
    );
    expect(popoverOutletRender).toHaveBeenCalledWith(
      expect.objectContaining({
        outlet: 'record_attribute_decoration',
        runtimeScope,
      }),
    );
    expect(drawerRender).toHaveBeenCalledWith(
      expect.objectContaining({ blueprintId, blueprintVersion: 7 }),
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

  it('shows data quality, comments and record control in tabs, one at a time', async () => {
    const user = userEvent.setup();
    const scrollIntoView = vi.fn();
    Element.prototype.scrollIntoView = scrollIntoView;
    const { currentSession } = await import('../auth/api');
    const api = await import('./api');
    const rules = await import('../rules/api');
    vi.mocked(currentSession).mockResolvedValue({
      capabilities: { rules_read: true },
    } as never);
    vi.mocked(rules.listRules).mockResolvedValue([]);
    vi.mocked(rules.listRecordRuleRuns).mockResolvedValue([]);
    vi.mocked(rules.listUnresolvedRecordFindings).mockResolvedValue({
      items: [],
    });
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
    renderPage();

    const tabs = within(await screen.findByRole('tablist')).getAllByRole('tab');
    expect(tabs.map((tab) => tab.textContent)).toEqual([
      'Data quality',
      'Comments',
      'Record control',
    ]);
    expect(
      (await screen.findByRole('tabpanel', { name: 'Data quality' }))
        .textContent,
    ).toContain('No open findings.');
    // Hidden panels stay mounted but out of the accessibility tree.
    expect(screen.getAllByRole('tabpanel')).toHaveLength(1);

    await user.click(screen.getByRole('tab', { name: 'Record control' }));
    expect(
      within(
        screen.getByRole('tabpanel', { name: 'Record control' }),
      ).getByText('No approvals or retention holds.'),
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
});
