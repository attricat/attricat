// @vitest-environment jsdom
import { render, screen, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { MenuList } from '@mui/material';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { toast } from '../../components/toast';
import { recordQueryKeys } from '../records/queryKeys';
import { useActionDialogStore } from './actionDialogStore';
import { ActionMenuCloseContext } from './actions';
import type { ExtensionContribution } from './api';

vi.mock('./api', async (importOriginal) => ({
  ...(await importOriginal<typeof import('./api')>()),
  getExtensionRuntime: vi.fn(),
  extensionCommand: vi.fn(),
}));
vi.mock('../../components/toast', () => ({
  toast: { show: vi.fn(), error: vi.fn() },
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
