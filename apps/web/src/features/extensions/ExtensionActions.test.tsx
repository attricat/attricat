// @vitest-environment jsdom
import { render, screen, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { MenuList } from '@mui/material';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { toast } from '../../components/toast';
import { recordQueryKeys } from '../records/queryKeys';
import { useActionConfirmStore } from './actionConfirmStore';
import { useActionDialogStore } from './actionDialogStore';
import i18n from '../../i18n';
import { startExtensionRun } from '../extension-runs/api';
import { notifyRunStarted } from '../extension-runs/runNotifications';
import { ActionMenuCloseContext } from './actions';
import type { ExtensionContribution } from './api';

vi.mock('./api', async (importOriginal) => ({
  ...(await importOriginal<typeof import('./api')>()),
  getExtensionRuntime: vi.fn(),
  extensionCommand: vi.fn(),
}));
vi.mock('../extension-runs/api', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../extension-runs/api')>()),
  startExtensionRun: vi.fn(),
}));
vi.mock('../extension-runs/runNotifications', () => ({
  notifyRunStarted: vi.fn(),
}));
vi.mock('@tanstack/react-router', () => ({ useNavigate: () => vi.fn() }));
vi.mock('../../components/toast', () => ({
  toast: { show: vi.fn(() => 1), error: vi.fn(), dismiss: vi.fn() },
}));

import { extensionCommand, getExtensionRuntime } from './api';
import {
  ExtensionActionButtons,
  ExtensionActionMenuItems,
} from './ExtensionActions';

const blueprintId = '22222222-2222-4222-8222-222222222222';
const recordId = '33333333-3333-4333-8333-333333333333';
const target = {
  blueprintId,
  blueprintVersion: 3,
  contextId: null,
  recordIds: [recordId],
};
const runtimeScope = { blueprintId, blueprintVersion: 3 };

const action = (
  id: string,
  invoke: ExtensionContribution['invoke'],
  overrides: Partial<ExtensionContribution> = {},
): ExtensionContribution => ({
  contribution_key: `acme.tools:${id}`,
  display_order: 0,
  navigation_group: null,
  capabilities: [],
  configuration: null,
  extension_id: 'acme.tools',
  extension_name: 'Acme tools',
  id,
  kind: 'action',
  outlet: 'explorer_row_action',
  release_id: '11111111-1111-4111-8111-111111111111',
  route: null,
  title: `Title ${id}`,
  version: 1,
  invoke,
  when: null,
  confirm: null,
  icon: null,
  translations: {},
  ...overrides,
});

const renderWithClient = (ui: React.ReactNode) => {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  const invalidate = vi.spyOn(client, 'invalidateQueries');
  render(<QueryClientProvider client={client}>{ui}</QueryClientProvider>);
  return { invalidate };
};

beforeEach(() => {
  vi.clearAllMocks();
  useActionDialogStore.getState().close();
});

describe('ExtensionActionMenuItems', () => {
  it("lists only the outlet's actions and opens a dialog with the action context", async () => {
    vi.mocked(getExtensionRuntime).mockResolvedValue([
      action('open', { dialog: 'documents' }),
      action(
        'elsewhere',
        { dialog: 'documents' },
        {
          outlet: 'explorer_bulk_action',
        },
      ),
      action('panel', null, { kind: 'panel' }),
    ]);
    const closeMenu = vi.fn();
    renderWithClient(
      <MenuList>
        <ActionMenuCloseContext value={closeMenu}>
          <ExtensionActionMenuItems
            outlet="explorer_row_action"
            runtimeScope={runtimeScope}
            target={target}
          />
        </ActionMenuCloseContext>
      </MenuList>,
    );

    await userEvent.click(
      await screen.findByRole('menuitem', { name: 'Title open' }),
    );
    expect(screen.getAllByRole('menuitem')).toHaveLength(1);
    expect(closeMenu).toHaveBeenCalledOnce();
    expect(useActionDialogStore.getState().dialog).toMatchObject({
      extensionId: 'acme.tools',
      dialogId: 'documents',
      context: {
        context_version: 1,
        action_id: 'open',
        outlet: 'explorer_row_action',
        blueprint_id: blueprintId,
        blueprint_version: 3,
        context_id: null,
        record_ids: [recordId],
      },
    });
  });

  it("calls the action's command and applies its notification and refresh", async () => {
    vi.mocked(getExtensionRuntime).mockResolvedValue([
      action('check', { command: 'check-formulas' }),
    ]);
    vi.mocked(extensionCommand).mockResolvedValue({
      notification: { message: 'All good.', severity: 'success' },
      refresh: true,
      formulas: [],
    });
    const { invalidate } = renderWithClient(
      <MenuList>
        <ExtensionActionMenuItems
          outlet="explorer_row_action"
          runtimeScope={runtimeScope}
          target={target}
        />
      </MenuList>,
    );

    await userEvent.click(
      await screen.findByRole('menuitem', { name: 'Title check' }),
    );

    expect(extensionCommand).toHaveBeenCalledWith('acme.tools', 'check', {
      release_id: '11111111-1111-4111-8111-111111111111',
      command_id: 'check-formulas',
      payload: expect.objectContaining({
        action_id: 'check',
        record_ids: [recordId],
      }),
    });
    await waitFor(() =>
      expect(toast.show).toHaveBeenCalledWith({
        message: 'All good.',
        severity: 'success',
      }),
    );
    await waitFor(() =>
      expect(invalidate).toHaveBeenCalledWith(
        { queryKey: recordQueryKeys.form(recordId) },
        { throwOnError: true },
      ),
    );
    expect(invalidate).toHaveBeenCalledWith({
      queryKey: recordQueryKeys.searches(),
    });
  });

  it('says which action failed when its command fails', async () => {
    vi.mocked(getExtensionRuntime).mockResolvedValue([
      action('check', { command: 'check-formulas' }),
    ]);
    vi.mocked(extensionCommand).mockRejectedValue(new Error('down'));
    renderWithClient(
      <MenuList>
        <ExtensionActionMenuItems
          outlet="explorer_row_action"
          runtimeScope={runtimeScope}
          target={target}
        />
      </MenuList>,
    );

    await userEvent.click(
      await screen.findByRole('menuitem', { name: 'Title check' }),
    );

    await waitFor(() =>
      expect(toast.error).toHaveBeenCalledWith(
        '“Title check” could not be completed.',
      ),
    );
  });
});

describe('ExtensionActionButtons', () => {
  it('shows four actions as buttons and the rest behind a menu', async () => {
    vi.mocked(getExtensionRuntime).mockResolvedValue(
      Array.from({ length: 5 }, (_, index) =>
        action(
          `a${index}`,
          { dialog: 'documents' },
          {
            outlet: 'record_action',
            display_order: index,
          },
        ),
      ),
    );
    renderWithClient(
      <ExtensionActionButtons
        outlet="record_action"
        runtimeScope={runtimeScope}
        target={target}
      />,
    );

    expect(
      await screen.findByRole('button', { name: 'Title a3' }),
    ).toBeTruthy();
    expect(screen.queryByRole('button', { name: 'Title a4' })).toBeNull();
    await userEvent.click(
      screen.getByRole('button', { name: 'More extension actions' }),
    );
    await userEvent.click(
      await screen.findByRole('menuitem', { name: 'Title a4' }),
    );
    expect(useActionDialogStore.getState().dialog?.context.action_id).toBe(
      'a4',
    );
  });
});

describe('action behaviours', () => {
  const renderRow = () =>
    renderWithClient(
      <MenuList>
        <ExtensionActionMenuItems
          outlet="explorer_row_action"
          runtimeScope={runtimeScope}
          target={target}
        />
      </MenuList>,
    );

  it('asks for confirmation first and runs nothing when declined', async () => {
    vi.mocked(getExtensionRuntime).mockResolvedValue([
      action(
        'wipe',
        { command: 'wipe' },
        { confirm: { message: 'Wipe it?', destructive: true } },
      ),
    ]);
    renderRow();
    await userEvent.click(
      await screen.findByRole('menuitem', { name: 'Title wipe' }),
    );
    expect(useActionConfirmStore.getState().pending).toMatchObject({
      title: 'Title wipe',
      message: 'Wipe it?',
      destructive: true,
    });
    useActionConfirmStore.getState().answer(false);
    await waitFor(() =>
      expect(useActionConfirmStore.getState().pending).toBeNull(),
    );
    expect(extensionCommand).not.toHaveBeenCalled();
  });

  it('starts its operation for the records and follows the run', async () => {
    vi.mocked(getExtensionRuntime).mockResolvedValue([
      action('run', {
        operation: { id: 'summarize', input: { template: 'a' } },
      }),
    ]);
    vi.mocked(startExtensionRun).mockResolvedValue({ run_id: 'run-1' });
    renderRow();
    await userEvent.click(
      await screen.findByRole('menuitem', { name: 'Title run' }),
    );
    await waitFor(() =>
      expect(startExtensionRun).toHaveBeenCalledWith(
        'acme.tools',
        'run',
        expect.objectContaining({
          operation_id: 'summarize',
          input: { template: 'a' },
          selection: expect.objectContaining({ record_ids: [recordId] }),
        }),
      ),
    );
    expect(notifyRunStarted).toHaveBeenCalledWith(
      'run-1',
      expect.any(Function),
      [recordId],
    );
  });

  it('opens a dialog a command names in its result', async () => {
    vi.mocked(getExtensionRuntime).mockResolvedValue([
      action('next', { command: 'next' }),
    ]);
    vi.mocked(extensionCommand).mockResolvedValue({ open_dialog: 'step-two' });
    renderRow();
    await userEvent.click(
      await screen.findByRole('menuitem', { name: 'Title next' }),
    );
    await waitFor(() =>
      expect(useActionDialogStore.getState().dialog).toMatchObject({
        dialogId: 'step-two',
        context: { action_id: 'next', record_ids: [recordId] },
      }),
    );
  });

  it('shows a translated title and hides actions whose conditions fail', async () => {
    vi.mocked(getExtensionRuntime).mockResolvedValue([
      action(
        'pl',
        { dialog: 'd' },
        { translations: { pl: { title: 'Po polsku' } } },
      ),
      action('many', { dialog: 'd' }, { when: { min_records: 2 } }),
    ]);
    await i18n.changeLanguage('pl');
    try {
      renderRow();
      expect(
        await screen.findByRole('menuitem', { name: 'Po polsku' }),
      ).toBeTruthy();
      expect(screen.getAllByRole('menuitem')).toHaveLength(1);
    } finally {
      await i18n.changeLanguage('en');
    }
  });
});
