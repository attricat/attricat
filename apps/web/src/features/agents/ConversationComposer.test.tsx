// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { uploadConversationFiles } from '../files/api';
import { sendMessage } from './api';
import { ConversationComposer } from './ConversationComposer';

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
});
