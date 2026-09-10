// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen, waitFor } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { ToastProvider } from '../../components/ToastProvider';
import { listContexts } from '../contexts/api';
import { EntityPreviewPage } from './EntityPreviewPage';

vi.mock('@tanstack/react-router', () => ({
  createLink: <T,>(component: T) => component,
  Link: ({ children }: { children: React.ReactNode }) => (
    <span>{children}</span>
  ),
}));

vi.mock('./api', () => ({
  getBlueprintRevision: vi.fn(),
  getCurrentBlueprint: vi.fn(),
  getResolvedEntityPreview: vi.fn(),
}));

vi.mock('../contexts/api', () => ({
  listContexts: vi.fn(),
}));

const renderPage = (relationshipPickerToken?: string) => {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });

  return render(
    <QueryClientProvider client={queryClient}>
      <ToastProvider>
        <EntityPreviewPage
          entityId="00000000-0000-4000-8000-000000000001"
          relationshipPickerToken={relationshipPickerToken}
        />
      </ToastProvider>
    </QueryClientProvider>,
  );
};

describe('EntityPreviewPage', () => {
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
      .getByRole('button', { name: 'Select this entity and close' })
      .click();

    expect(postMessage).toHaveBeenCalledWith(
      {
        type: 'attricat.relationship-picker.select',
        token: 'picker-token',
        entityId: '00000000-0000-4000-8000-000000000001',
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
