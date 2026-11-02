// @vitest-environment jsdom
import { MenuItem } from '@mui/material';
import { act, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { useActionDialogStore } from '../extensions/actionDialogStore';
import { ExplorerSelectionActionsMenu } from './ExplorerSelectionActionsMenu';

const access = vi.hoisted(() => ({
  canRun: false,
  unavailableReason: undefined as string | undefined,
}));
vi.mock('../auth/useCan', () => ({ useCan: () => access.canRun }));
vi.mock('../agents/useAgentsUnavailableReason', () => ({
  useAgentsUnavailableReason: () => access.unavailableReason,
}));

describe('ExplorerSelectionActionsMenu', () => {
  it('offers extension actions and hands off to the dialog one opens', async () => {
    const user = userEvent.setup();
    render(
      <ExplorerSelectionActionsMenu
        disabled={false}
        extensionActions={<MenuItem>More actions…</MenuItem>}
        onSaveAsSearch={vi.fn()}
        onSendToAgent={vi.fn()}
      />,
    );
    await user.click(screen.getByRole('button', { name: 'Actions' }));
    expect(
      screen.getByRole('menuitem', { name: 'More actions…' }),
    ).toBeTruthy();

    act(() =>
      useActionDialogStore.getState().open({ title: 'Recalculate' } as never),
    );

    await waitFor(() => expect(screen.queryByRole('menu')).toBeNull());
    act(() => useActionDialogStore.getState().close());
  });

  it('disables sending to an agent when no provider is configured', async () => {
    access.canRun = true;
    access.unavailableReason = 'No provider';
    const user = userEvent.setup();
    const onSendToAgent = vi.fn();
    render(
      <ExplorerSelectionActionsMenu
        disabled={false}
        onSaveAsSearch={vi.fn()}
        onSendToAgent={onSendToAgent}
      />,
    );
    await user.click(screen.getByRole('button', { name: 'Actions' }));

    expect(
      screen
        .getByRole('menuitem', { name: 'Send to agent conversation' })
        .getAttribute('aria-disabled'),
    ).toBe('true');
    access.canRun = false;
    access.unavailableReason = undefined;
  });
});
