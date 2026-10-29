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
  setConversationAutoApproval,
} from '../../agents/api';
import type { DraftContext } from '../../agents/ConversationPanel';
import { RecordAgentDrawer } from './RecordAgentDrawer';

vi.mock('../../agents/api', () => ({
  createConversation: vi.fn(),
  getConversation: vi.fn(),
  sendMessage: vi.fn(),
  setConversationAutoApproval: vi.fn(),
}));
vi.mock('../../auth/api', () => ({
  currentSession: async () => ({
    user_id: '123e4567-e89b-12d3-a456-426614174010',
    workspace_id: '123e4567-e89b-12d3-a456-426614174011',
  }),
}));
vi.mock('../../agents/ConversationPanel', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../../agents/ConversationPanel')>()),
  ConversationPanel: ({
    conversationId,
    settings,
  }: {
    conversationId: string;
    settings?: React.ReactNode;
  }) => (
    <>
      <p>Transcript of {conversationId}</p>
      {settings}
    </>
  ),
}));

const recordId = '123e4567-e89b-12d3-a456-426614174000';
const contextId = '123e4567-e89b-12d3-a456-426614174001';
const conversationId = '123e4567-e89b-12d3-a456-426614174002';

const Drawer = ({
  draft,
  initialConversation = null,
  onShowAll,
}: {
  draft?: DraftContext;
  initialConversation?: string | null;
  onShowAll?: () => void;
}) => {
  const [conversation, setConversation] = useState(initialConversation);
  return (
    <RecordAgentDrawer
      contextId={contextId}
      conversationId={conversation}
      draft={draft}
      onClose={vi.fn()}
      onConversationChange={setConversation}
      onShowAll={onShowAll}
      open
      recordId={recordId}
    />
  );
};

const renderDrawer = (
  onShowAll?: () => void,
  options: { draft?: DraftContext; initialConversation?: string } = {},
) =>
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
      <Drawer onShowAll={onShowAll} {...options} />
    </QueryClientProvider>,
  );

describe('RecordAgentDrawer', () => {
  beforeEach(() => {
    vi.mocked(createConversation).mockReset();
    vi.mocked(getConversation).mockReset();
    vi.mocked(sendMessage).mockReset();
    vi.mocked(setConversationAutoApproval).mockReset();
  });

  const draft: DraftContext = {
    recordId,
    contextId,
    defaultContextId: contextId,
    getDraftValues: () => ({ title: 'Trail shoe' }),
    onApply: vi.fn(),
  };

  it('switches auto-approval on for a new conversation before its first message', async () => {
    vi.mocked(createConversation).mockResolvedValue({
      id: conversationId,
      title: 'Set the price',
    } as never);
    vi.mocked(setConversationAutoApproval).mockResolvedValue({} as never);
    vi.mocked(sendMessage).mockResolvedValue({
      id: '123e4567-e89b-12d3-a456-426614174003',
      status: 'queued',
    });
    vi.mocked(getConversation).mockResolvedValue({
      id: conversationId,
      title: 'Set the price',
      record_id: recordId,
      context_id: contextId,
      auto_approve_user_id: '123e4567-e89b-12d3-a456-426614174010',
    } as never);
    const user = userEvent.setup();
    renderDrawer(undefined, { draft });

    await user.click(
      screen.getByRole('switch', {
        name: 'Auto-approve changes within this conversation',
      }),
    );
    await user.type(
      screen.getByRole('textbox', { name: 'Message' }),
      'Set the price{Enter}',
    );

    await waitFor(() =>
      expect(sendMessage).toHaveBeenCalledWith(
        conversationId,
        'Set the price',
        [],
        {
          title: 'Trail shoe',
        },
      ),
    );
    expect(setConversationAutoApproval).toHaveBeenCalledWith(
      conversationId,
      true,
    );
    expect(
      vi.mocked(setConversationAutoApproval).mock.invocationCallOrder[0],
    ).toBeLessThan(vi.mocked(sendMessage).mock.invocationCallOrder[0]);
  });

  it('says when auto-approval is on for someone else and takes it over', async () => {
    vi.mocked(getConversation).mockResolvedValue({
      id: conversationId,
      title: 'Prices',
      record_id: recordId,
      context_id: contextId,
      auto_approve_user_id: '123e4567-e89b-12d3-a456-426614174099',
    } as never);
    vi.mocked(setConversationAutoApproval).mockResolvedValue({} as never);
    const user = userEvent.setup();
    renderDrawer(undefined, { draft, initialConversation: conversationId });

    const about = await screen.findByRole('button', {
      name: 'About auto-approval',
    });
    await user.hover(about);
    expect(
      await screen.findByText(/On for someone else's messages/),
    ).toBeTruthy();
    const toggle = screen.getByRole('switch', {
      name: 'Auto-approve changes within this conversation',
    });
    expect((toggle as HTMLInputElement).checked).toBe(false);
    // The info button sits in the switch's label but does not flip it.
    await user.click(about);
    expect((toggle as HTMLInputElement).checked).toBe(false);
    expect(setConversationAutoApproval).not.toHaveBeenCalled();
    await waitFor(() =>
      expect((toggle as HTMLInputElement).disabled).toBe(false),
    );
    await user.click(toggle);
    expect(setConversationAutoApproval).toHaveBeenCalledWith(
      conversationId,
      true,
    );
  });

  it('offers no auto-approval to someone who cannot edit the record', async () => {
    renderDrawer();
    await screen.findByRole('textbox', { name: 'Message' });
    expect(
      screen.queryByRole('switch', {
        name: 'Auto-approve changes within this conversation',
      }),
    ).toBeNull();
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
