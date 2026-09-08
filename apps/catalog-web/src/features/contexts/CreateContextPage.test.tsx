// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen, waitFor } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { listContexts } from './api';
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
