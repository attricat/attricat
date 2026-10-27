// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { clearBudget, listBudgets } from './api';
import { ExtensionBudgetsSection } from './ExtensionBudgetsSection';

vi.mock('./api', () => ({
  listBudgets: vi.fn(),
  setBudget: vi.fn(),
  clearBudget: vi.fn(async () => undefined),
}));

const declared = { requests: 100, per_seconds: 60 };

describe('ExtensionBudgetsSection', () => {
  it('shows the declared budget in its inputs after a reset', async () => {
    vi.mocked(listBudgets)
      .mockResolvedValueOnce([
        {
          host_permission_id: 'shop-api',
          declared,
          lowered: { requests: 10, per_seconds: 60 },
        },
      ])
      .mockResolvedValue([
        { host_permission_id: 'shop-api', declared, lowered: null },
      ]);
    const user = userEvent.setup();
    render(
      <QueryClientProvider client={new QueryClient()}>
        <ExtensionBudgetsSection canManage extensionId="acme.shop" />
      </QueryClientProvider>,
    );
    const requests = await screen.findByRole('spinbutton', {
      name: 'Requests',
    });
    expect((requests as HTMLInputElement).value).toBe('10');

    await user.click(screen.getByRole('button', { name: 'Reset' }));

    expect(clearBudget).toHaveBeenCalledWith('acme.shop', 'shop-api');
    await waitFor(() =>
      expect(screen.queryByRole('button', { name: 'Reset' })).toBeNull(),
    );
    expect((requests as HTMLInputElement).value).toBe('100');
  });
});
