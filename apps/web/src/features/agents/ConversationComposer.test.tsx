// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { uploadConversationFiles } from '../files/api';
import { sendMessage } from './api';
import { ConversationComposer } from './ConversationComposer';
import { agentQueryKeys } from './queryKeys';

vi.mock('../files/api', () => ({
  uploadConversationFiles: vi.fn(),
}));

vi.mock('./api', () => ({
  sendMessage: vi.fn(),
}));

const conversationId = '123e4567-e89b-12d3-a456-426614174000';

const renderComposer = (
  onSent = vi.fn(),
  onSendingChange = vi.fn(),
  sendDraft?: (
    content: string,
    conversationId: string,
    attachmentIds: string[],
  ) => Promise<unknown>,
) => {
  const queryClient = new QueryClient({
    defaultOptions: { mutations: { retry: false } },
  });

  render(
    <QueryClientProvider client={queryClient}>
      <ConversationComposer
        conversationId={conversationId}
        onSendingChange={onSendingChange}
        onSent={onSent}
        sendDraft={sendDraft}
      />
    </QueryClientProvider>,
  );

  return { onSendingChange, onSent };
};

describe('ConversationComposer', () => {
  beforeEach(() => {
    vi.mocked(sendMessage).mockReset();
    vi.mocked(uploadConversationFiles).mockReset();
  });

  it('uploads attachments before sending and resets after a successful send', async () => {
    const user = userEvent.setup();
    const { onSendingChange, onSent } = renderComposer();
    const file = new File(['catalog'], 'attricat.csv', { type: 'text/csv' });

    vi.mocked(uploadConversationFiles).mockResolvedValue({
      files: [{ id: '123e4567-e89b-12d3-a456-426614174001' }],
    } as Awaited<ReturnType<typeof uploadConversationFiles>>);
    vi.mocked(sendMessage).mockResolvedValue({
      id: '123e4567-e89b-12d3-a456-426614174002',
      status: 'queued',
    });

    await user.type(screen.getByRole('textbox', { name: 'Message' }), 'Review');
    await user.upload(document.querySelector('input[type="file"]')!, file);
    await user.click(screen.getByRole('button', { name: 'Send' }));

    await waitFor(() =>
      expect(uploadConversationFiles).toHaveBeenCalledWith(conversationId, [
        file,
      ]),
    );
    expect(sendMessage).toHaveBeenCalledWith(conversationId, 'Review', [
      '123e4567-e89b-12d3-a456-426614174001',
    ]);
    await waitFor(() => expect(onSent).toHaveBeenCalledOnce());
    expect(onSendingChange).toHaveBeenNthCalledWith(1, true);
    expect(onSendingChange).toHaveBeenLastCalledWith(false);
    expect(screen.queryByText('attricat.csv')).toBeNull();
    expect(screen.getByRole('textbox', { name: 'Message' })).toHaveProperty(
      'value',
      '',
    );
  });

  it('locks the draft while uploading and sends the submitted snapshot', async () => {
    const user = userEvent.setup();
    renderComposer();
    const file = new File(['data'], 'data.csv');
    let finishUpload!: (
      value: Awaited<ReturnType<typeof uploadConversationFiles>>,
    ) => void;
    vi.mocked(uploadConversationFiles).mockImplementation(
      () =>
        new Promise((resolve) => {
          finishUpload = resolve;
        }),
    );
    vi.mocked(sendMessage).mockResolvedValue({
      id: '123e4567-e89b-12d3-a456-426614174002',
      status: 'queued',
    });
    const message = screen.getByRole('textbox', { name: 'Message' });
    await user.type(message, 'Original');
    await user.upload(document.querySelector('input[type="file"]')!, file);
    await user.click(screen.getByRole('button', { name: 'Send' }));
    await waitFor(() => expect(uploadConversationFiles).toHaveBeenCalledOnce());
    expect(message.hasAttribute('disabled')).toBe(true);
    expect(
      screen
        .getByRole('button', { name: 'Add files' })
        .hasAttribute('disabled'),
    ).toBe(true);
    finishUpload({
      files: [{ id: '123e4567-e89b-12d3-a456-426614174001' }],
    } as never);
    await waitFor(() =>
      expect(sendMessage).toHaveBeenCalledWith(conversationId, 'Original', [
        '123e4567-e89b-12d3-a456-426614174001',
      ]),
    );
  });

  it('reuses the composer and uploaded attachments for draft proposals', async () => {
    const user = userEvent.setup();
    const sendDraft = vi.fn().mockResolvedValue({ fields: {} });
    renderComposer(vi.fn(), vi.fn(), sendDraft);
    vi.mocked(uploadConversationFiles).mockResolvedValue({
      files: [{ id: '123e4567-e89b-12d3-a456-426614174001' }],
    } as never);
    await user.upload(
      document.querySelector('input[type="file"]')!,
      new File(['spec'], 'spec.txt', { type: 'text/plain' }),
    );
    await user.click(screen.getByRole('button', { name: 'Send' }));
    await waitFor(() =>
      expect(sendDraft).toHaveBeenCalledWith('', conversationId, [
        '123e4567-e89b-12d3-a456-426614174001',
      ]),
    );
    expect(sendMessage).not.toHaveBeenCalled();
  });

  it('submits a message from Enter', async () => {
    const user = userEvent.setup();
    renderComposer();
    vi.mocked(sendMessage).mockResolvedValue({
      id: '123e4567-e89b-12d3-a456-426614174002',
      status: 'queued',
    });
    const message = screen.getByRole('textbox', { name: 'Message' });

    await user.type(message, 'Review');
    await user.keyboard('{Enter}');

    await waitFor(() =>
      expect(sendMessage).toHaveBeenCalledWith(conversationId, 'Review', []),
    );
  });

  it('refreshes but does not lead on after it is gone mid-send', async () => {
    const user = userEvent.setup();
    const onSent = vi.fn();
    const queryClient = new QueryClient({
      defaultOptions: { mutations: { retry: false } },
    });
    const invalidate = vi.spyOn(queryClient, 'invalidateQueries');
    let finishSend!: (value: Awaited<ReturnType<typeof sendMessage>>) => void;
    vi.mocked(sendMessage).mockImplementation(
      () =>
        new Promise((resolve) => {
          finishSend = resolve;
        }),
    );
    const { unmount } = render(
      <QueryClientProvider client={queryClient}>
        <ConversationComposer conversationId={conversationId} onSent={onSent} />
      </QueryClientProvider>,
    );
    await user.type(screen.getByRole('textbox', { name: 'Message' }), 'Hi');
    await user.click(screen.getByRole('button', { name: 'Send' }));
    await waitFor(() => expect(sendMessage).toHaveBeenCalledOnce());

    unmount();
    finishSend({
      id: '123e4567-e89b-12d3-a456-426614174002',
      status: 'queued',
    });

    await waitFor(() =>
      expect(invalidate).toHaveBeenCalledWith({
        queryKey: agentQueryKeys.conversation(conversationId),
      }),
    );
    expect(onSent).not.toHaveBeenCalled();
  });

  it('creates the conversation on the first send and reuses it on retry', async () => {
    const user = userEvent.setup();
    const onSent = vi.fn();
    const createConversation = vi.fn().mockResolvedValue(conversationId);
    render(
      <QueryClientProvider
        client={
          new QueryClient({ defaultOptions: { mutations: { retry: false } } })
        }
      >
        <ConversationComposer
          conversationId={null}
          createConversation={createConversation}
          onSendingChange={vi.fn()}
          onSent={onSent}
        />
      </QueryClientProvider>,
    );
    vi.mocked(sendMessage)
      .mockRejectedValueOnce(new Error('offline'))
      .mockResolvedValueOnce({
        id: '123e4567-e89b-12d3-a456-426614174002',
        status: 'queued',
      });
    await user.type(screen.getByRole('textbox', { name: 'Message' }), 'Hi');
    await user.click(screen.getByRole('button', { name: 'Send' }));
    await user.click(await screen.findByRole('button', { name: 'Retry' }));
    await waitFor(() => expect(onSent).toHaveBeenCalledWith(conversationId));
    expect(createConversation).toHaveBeenCalledOnce();
    expect(createConversation).toHaveBeenCalledWith('Hi');
    expect(sendMessage).toHaveBeenLastCalledWith(conversationId, 'Hi', []);
  });

  it('lists a created conversation even when its first message fails', async () => {
    const user = userEvent.setup();
    const queryClient = new QueryClient({
      defaultOptions: { mutations: { retry: false } },
    });
    const invalidate = vi.spyOn(queryClient, 'invalidateQueries');
    render(
      <QueryClientProvider client={queryClient}>
        <ConversationComposer
          conversationId={null}
          createConversation={vi.fn().mockResolvedValue(conversationId)}
        />
      </QueryClientProvider>,
    );
    vi.mocked(sendMessage).mockRejectedValueOnce(new Error('offline'));
    await user.type(screen.getByRole('textbox', { name: 'Message' }), 'Hi');
    await user.click(screen.getByRole('button', { name: 'Send' }));

    expect(await screen.findByRole('button', { name: 'Retry' })).toBeTruthy();
    expect(invalidate).toHaveBeenCalledWith({
      queryKey: agentQueryKeys.conversations(),
    });
  });
});
