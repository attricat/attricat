// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { getConversation } from './api';
import { ConversationDetailPage } from './ConversationDetailPage';

vi.mock('./api', () => ({
  getConversation: vi.fn(),
  updateConversationTitle: vi.fn(),
}));
vi.mock('./ConversationPanel', () => ({ ConversationPanel: () => null }));
const conversationId = '123e4567-e89b-12d3-a456-426614174000';

describe('ConversationDetailPage', () => {
  it('offers a rename action for the loaded conversation', async () => {
    vi.mocked(getConversation).mockResolvedValue({
      id: conversationId,
      title: 'Original title',
      title_source: 'generated',
      updated_at: '2026-09-28T00:00:00Z',
    } as never);
    const user = userEvent.setup();
    render(
      <QueryClientProvider
        client={
          new QueryClient({ defaultOptions: { queries: { retry: false } } })
        }
      >
        <ConversationDetailPage conversationId={conversationId} />
      </QueryClientProvider>,
    );
    await user.click(
      await screen.findByRole('button', { name: 'Change conversation title' }),
    );
    await waitFor(() =>
      expect(document.title).toBe('Agent conversation · Original title · Attricat'),
    );
    expect(screen.getByRole('dialog')).toBeTruthy();
    expect(
      screen.getByRole('textbox', { name: 'Conversation title' }),
    ).toHaveProperty('value', 'Original title');
  });
});
