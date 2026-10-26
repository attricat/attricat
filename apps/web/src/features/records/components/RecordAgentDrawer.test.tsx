// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { useState } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import '../../../i18n';
import {
  createConversation,
  getConversation,
  sendMessage,
} from '../../agents/api';
import { RecordAgentDrawer } from './RecordAgentDrawer';

vi.mock('../../agents/api', () => ({
  createConversation: vi.fn(),
  getConversation: vi.fn(),
  sendMessage: vi.fn(),
}));
vi.mock('../../agents/ConversationPanel', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../../agents/ConversationPanel')>()),
  ConversationPanel: ({ conversationId }: { conversationId: string }) => (
    <p>Transcript of {conversationId}</p>
  ),
}));

const recordId = '123e4567-e89b-12d3-a456-426614174000';
const contextId = '123e4567-e89b-12d3-a456-426614174001';
const conversationId = '123e4567-e89b-12d3-a456-426614174002';

const Drawer = ({ onShowAll }: { onShowAll?: () => void }) => {
  const [conversation, setConversation] = useState<string | null>(null);
  return (
    <RecordAgentDrawer
      contextId={contextId}
      conversationId={conversation}
      onClose={vi.fn()}
      onConversationChange={setConversation}
      onShowAll={onShowAll}
      open
      recordId={recordId}
    />
  );
};

const renderDrawer = (onShowAll?: () => void) =>
  render(
    <QueryClientProvider
      client={
        new QueryClient({
          defaultOptions: {
            queries: { retry: false },
            mutations: { retry: false },
          },
        })
      }
    >
      <Drawer onShowAll={onShowAll} />
    </QueryClientProvider>,
  );

describe('RecordAgentDrawer', () => {
  beforeEach(() => {
    vi.mocked(createConversation).mockReset();
    vi.mocked(getConversation).mockReset();
    vi.mocked(sendMessage).mockReset();
  });

  it('opens on a focused message box and creates the conversation on send', async () => {
    vi.mocked(createConversation).mockResolvedValue({
      id: conversationId,
      title: 'Which sizes are missing?',
    });
    vi.mocked(sendMessage).mockResolvedValue({
      id: '123e4567-e89b-12d3-a456-426614174003',
      status: 'queued',
    });
    vi.mocked(getConversation).mockResolvedValue({
      id: conversationId,
      title: 'Which sizes are missing?',
      title_source: 'pending',
      record_id: recordId,
      context_id: contextId,
    } as never);
    const user = userEvent.setup();
    renderDrawer();
    const message = screen.getByRole('textbox', { name: 'Message' });
    await waitFor(() => expect(document.activeElement).toBe(message));
    expect(createConversation).not.toHaveBeenCalled();
    await user.type(message, 'Which sizes are missing?{Enter}');
    expect(
      await screen.findByText(`Transcript of ${conversationId}`),
    ).toBeTruthy();
    expect(createConversation).toHaveBeenCalledWith(
      'Which sizes are missing?',
      { record_id: recordId, context_id: contextId },
    );
    expect(sendMessage).toHaveBeenCalledWith(
      conversationId,
      'Which sizes are missing?',
      [],
    );
    await user.click(screen.getByRole('button', { name: 'New conversation' }));
    expect(screen.getByRole('textbox', { name: 'Message' })).toBeTruthy();
  });

  it('leads to the record conversations', async () => {
    const onShowAll = vi.fn();
    const user = userEvent.setup();
    renderDrawer(onShowAll);
    await user.click(
      screen.getByRole('button', {
        name: 'All conversations about this record',
      }),
    );
    expect(onShowAll).toHaveBeenCalledOnce();
  });
});
