// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { listSavedViews } from './api';
import { SavedSearchesDialog } from './SavedSearchesDialog';
import type { SavedSearchActions } from './useSavedSearchActions';
import type { SavedView } from './schemas';

vi.mock('./api', () => ({ listSavedViews: vi.fn() }));

const view = {
  id: '00000000-0000-4000-8000-000000000001',
  owner_user_id: '00000000-0000-4000-8000-000000000002',
  kind: 'explorer_search',
  name: 'Chairs',
  description: null,
  visibility: 'private',
  state: { blueprint: 'product' },
  created_at: '2026-10-20T10:00:00Z',
  updated_at: '2026-10-20T10:00:00Z',
} as SavedView;

describe('SavedSearchesDialog', () => {
  it('asks before deleting a saved search', async () => {
    vi.mocked(listSavedViews).mockResolvedValue([view]);
    const remove = vi.fn(async () => undefined);
    const actions = {
      busy: false,
      canDelete: () => true,
      canEdit: false,
      isDraft: false,
      remove,
    } as unknown as SavedSearchActions;
    const user = userEvent.setup();
    render(
      <QueryClientProvider client={new QueryClient()}>
        <SavedSearchesDialog
          actions={actions}
          onClose={vi.fn()}
          open
          search={{ blueprint: 'product' }}
        />
      </QueryClientProvider>,
    );

    await user.click(
      await screen.findByRole('button', { name: /Delete saved search Chairs/ }),
    );
    expect(remove).not.toHaveBeenCalled();
    expect(screen.getByText('Delete the saved search “Chairs”?')).toBeTruthy();
    await user.click(screen.getByRole('button', { name: 'Delete' }));

    expect(remove).toHaveBeenCalledWith(view.id);
    await waitFor(() =>
      expect(
        screen.queryByText('Delete the saved search “Chairs”?'),
      ).toBeNull(),
    );
  });
});
