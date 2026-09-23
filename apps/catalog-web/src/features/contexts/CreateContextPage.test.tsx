// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { createContext, listContexts } from './api';
import { CreateContextPage } from './CreateContextPage';

vi.mock('@tanstack/react-router', () => ({
  Link: ({ children }: { children: React.ReactNode }) => (
    <span>{children}</span>
  ),
  useNavigate: () => vi.fn(),
}));

vi.mock('./api', () => ({
  createContext: vi.fn(),
  listContexts: vi.fn(),
}));

const renderPage = () => {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });

  return render(
    <QueryClientProvider client={queryClient}>
      <CreateContextPage />
    </QueryClientProvider>,
  );
};

describe('CreateContextPage', () => {
  it('keeps the submitted context fields fixed until creation finishes', async () => {
    vi.mocked(listContexts).mockResolvedValue([
      { id: 'parent-id', code: 'default', data: {}, parent_id: null },
    ]);
    let finish!: (value: Awaited<ReturnType<typeof createContext>>) => void;
    vi.mocked(createContext).mockImplementation(
      () =>
        new Promise((resolve) => {
          finish = resolve;
        }),
    );
    renderPage();
    const user = userEvent.setup();
    await user.type(screen.getByRole('textbox', { name: 'Code' }), 'example');
    await user.click(
      await screen.findByRole('combobox', { name: 'Parent context' }),
    );
    await user.click(screen.getByRole('option', { name: 'default' }));
    await user.click(screen.getByRole('button', { name: 'Create context' }));
    expect(createContext).toHaveBeenCalledWith('example', {}, 'parent-id');
    expect(
      (screen.getByRole('textbox', { name: 'Code' }) as HTMLInputElement)
        .disabled,
    ).toBe(true);
    expect(
      (screen.getByRole('textbox', { name: 'Metadata' }) as HTMLTextAreaElement)
        .disabled,
    ).toBe(true);
    expect(
      screen
        .getByRole('combobox', { name: 'Parent context' })
        .getAttribute('aria-disabled'),
    ).toBe('true');
    finish({} as Awaited<ReturnType<typeof createContext>>);
  });

  it('shows a retryable error and disables context-dependent controls when contexts fail to load', async () => {
    vi.mocked(listContexts).mockRejectedValue(
      new Error('contexts unavailable'),
    );

    renderPage();

    expect(
      await screen.findByText('Unable to load this information'),
    ).toBeTruthy();
    expect(
      screen
        .getByRole('combobox', { name: 'Parent context' })
        .getAttribute('aria-disabled'),
    ).toBe('true');
    expect(
      (
        screen.getByRole('button', {
          name: 'Create context',
        }) as HTMLButtonElement
      ).disabled,
    ).toBe(true);

    screen.getByRole('button', { name: 'Try again' }).click();

    await waitFor(() => expect(listContexts).toHaveBeenCalledTimes(2));
  });
});
