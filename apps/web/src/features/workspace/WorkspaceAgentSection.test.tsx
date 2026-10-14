// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { getAgentSettings, updateAgentSettings } from './api';
import { WorkspaceAgentSection } from './WorkspaceAgentSection';

vi.mock('./api', () => ({
  getAgentSettings: vi.fn(),
  updateAgentSettings: vi.fn(),
}));

const renderSection = (canManage = true) =>
  render(
    <QueryClientProvider
      client={
        new QueryClient({ defaultOptions: { queries: { retry: false } } })
      }
    >
      <WorkspaceAgentSection canManage={canManage} />
    </QueryClientProvider>,
  );

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(getAgentSettings).mockResolvedValue({
    bulk_edits: 'per_proposal',
    bulk_edit_max_records: 500,
  });
});

describe('workspace agent settings', () => {
  it('saves the bulk edit policy and record limit', async () => {
    vi.mocked(updateAgentSettings).mockImplementation(
      async (settings) => settings,
    );
    renderSection();
    const perProposal = await screen.findByRole('radio', {
      name: /Approve once per proposal/,
    });
    expect((perProposal as HTMLInputElement).checked).toBe(true);
    await userEvent.click(
      screen.getByRole('radio', { name: /Approve each record/ }),
    );
    const limit = screen.getByRole('spinbutton', {
      name: 'Records per bulk edit',
    });
    await userEvent.clear(limit);
    await userEvent.type(limit, '120');
    await userEvent.click(
      screen.getByRole('button', { name: 'Save agent settings' }),
    );
    await waitFor(() => expect(updateAgentSettings).toHaveBeenCalled());
    expect(vi.mocked(updateAgentSettings).mock.calls[0][0]).toEqual({
      bulk_edits: 'per_record',
      bulk_edit_max_records: 120,
    });
    expect(await screen.findByText('Agent settings saved.')).toBeTruthy();
  });

  it('rejects a limit outside the allowed range without saving', async () => {
    renderSection();
    const limit = await screen.findByRole('spinbutton', {
      name: 'Records per bulk edit',
    });
    await userEvent.clear(limit);
    await userEvent.type(limit, '5001');
    await userEvent.click(
      screen.getByRole('button', { name: 'Save agent settings' }),
    );
    expect(
      await screen.findByText('Enter a whole number from 1 to 5000.'),
    ).toBeTruthy();
    expect(updateAgentSettings).not.toHaveBeenCalled();
  });

  it('explains when the user may not manage agent settings', () => {
    renderSection(false);
    expect(
      screen.getByText('You are not authorized to manage agent settings.'),
    ).toBeTruthy();
    expect(getAgentSettings).not.toHaveBeenCalled();
  });
});
