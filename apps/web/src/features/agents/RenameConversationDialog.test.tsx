// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { updateConversationTitle } from './api';
import { RenameConversationDialog } from './RenameConversationDialog';

vi.mock('./api', () => ({ updateConversationTitle: vi.fn() }));
const conversationId = '123e4567-e89b-12d3-a456-426614174000';
const renderDialog = (onClose = vi.fn()) => {
  const client = new QueryClient({
    defaultOptions: { mutations: { retry: false } },
  });
  const invalidate = vi.spyOn(client, 'invalidateQueries');
  render(
    <QueryClientProvider client={client}>
      <RenameConversationDialog
        conversationId={conversationId}
        initialTitle="Old title"
        onClose={onClose}
      />
    </QueryClientProvider>,
  );
  return { onClose, invalidate };
};

describe('RenameConversationDialog', () => {
  it('saves a trimmed title and refreshes conversation queries', async () => {
    const user = userEvent.setup();
    const { onClose, invalidate } = renderDialog();
    vi.mocked(updateConversationTitle).mockResolvedValue({
      title: 'New title',
    } as never);
    await user.clear(
      screen.getByRole('textbox', { name: 'Conversation title' }),
    );
    await user.type(
      screen.getByRole('textbox', { name: 'Conversation title' }),
      '  New title  ',
    );
    await user.click(screen.getByRole('button', { name: 'Save title' }));
    await waitFor(() =>
      expect(updateConversationTitle).toHaveBeenCalledWith(
        conversationId,
        'New title',
      ),
    );
    await waitFor(() =>
      expect(invalidate).toHaveBeenCalledWith({
        queryKey: ['agents', 'conversations'],
      }),
    );
    expect(onClose).toHaveBeenCalledOnce();
  });

  it('rejects an empty title without making a request', async () => {
    const user = userEvent.setup();
    renderDialog();
    vi.mocked(updateConversationTitle).mockClear();
    await user.clear(
      screen.getByRole('textbox', { name: 'Conversation title' }),
    );
    await user.click(screen.getByRole('button', { name: 'Save title' }));
    expect(await screen.findByText('Enter a conversation title.')).toBeTruthy();
    expect(updateConversationTitle).not.toHaveBeenCalled();
  });
});
