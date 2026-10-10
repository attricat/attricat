// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { createConversation, sendMessage } from './api';
import { uploadConversationFiles } from '../files/api';
import { NewConversationPage } from './NewConversationPage';

vi.mock('@tanstack/react-router', () => ({ useNavigate: () => vi.fn() }));
vi.mock('./api', () => ({ createConversation: vi.fn(), sendMessage: vi.fn() }));
vi.mock('../files/api', () => ({ uploadConversationFiles: vi.fn() }));

describe('NewConversationPage', () => {
  it('keeps the first message stable through conversation creation and file upload', async () => {
    const user = userEvent.setup();
    let finishCreation!: (
      value: Awaited<ReturnType<typeof createConversation>>,
    ) => void;
    vi.mocked(createConversation).mockImplementation(
      () =>
        new Promise((resolve) => {
          finishCreation = resolve;
        }),
    );
    vi.mocked(uploadConversationFiles).mockResolvedValue({
      files: [],
    } as never);
    vi.mocked(sendMessage).mockResolvedValue({
      id: 'message',
      status: 'queued',
    } as never);
    render(
      <QueryClientProvider client={new QueryClient()}>
        <NewConversationPage />
      </QueryClientProvider>,
    );
    const message = screen.getByRole('textbox', { name: 'Message' });
    await user.type(message, 'Inspect this file');
    await user.upload(
      document.querySelector('input[type="file"]')!,
      new File(['x'], 'x.txt'),
    );
    await user.click(screen.getByRole('button', { name: 'Send' }));
    await waitFor(() => expect(createConversation).toHaveBeenCalledOnce());
    expect(message.hasAttribute('disabled')).toBe(true);
    expect(
      screen
        .getByRole('button', { name: 'Add files' })
        .hasAttribute('disabled'),
    ).toBe(true);
    finishCreation({ id: 'conversation' } as never);
    await waitFor(() =>
      expect(sendMessage).toHaveBeenCalledWith(
        'conversation',
        'Inspect this file',
        [],
      ),
    );
  });
});
