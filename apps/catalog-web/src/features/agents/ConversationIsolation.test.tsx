// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { ConversationDetailPage } from './ConversationDetailPage';
import {
  getConversation,
  listApprovals,
  listMessages,
  listRuns,
  sendMessage,
} from './api';
import { uploadConversationFiles } from '../files/api';

vi.mock('./api', () => ({
  getConversation: vi.fn(async (id: string) => ({
    id,
    title: id,
    title_source: 'user',
  })),
  listRuns: vi.fn().mockResolvedValue([]),
  listMessages: vi.fn().mockResolvedValue([]),
  listApprovals: vi.fn().mockResolvedValue([]),
  sendMessage: vi.fn(),
}));
vi.mock('../files/api', () => ({ uploadConversationFiles: vi.fn() }));

const first = '123e4567-e89b-12d3-a456-426614174000';
const second = '123e4567-e89b-12d3-a456-426614174001';

describe('conversation identity boundary', () => {
  it('does not carry failed messages or uploaded attachments into another conversation', async () => {
    vi.mocked(getConversation).mockImplementation(
      async (id) => ({ id, title: id, title_source: 'user' }) as never,
    );
    vi.mocked(listRuns).mockResolvedValue([]);
    vi.mocked(listMessages).mockResolvedValue([]);
    vi.mocked(listApprovals).mockResolvedValue([]);
    Element.prototype.scrollIntoView = vi.fn();
    const user = userEvent.setup();
    const client = new QueryClient({
      defaultOptions: {
        queries: { retry: false },
        mutations: { retry: false },
      },
    });
    const tree = (id: string) => (
      <QueryClientProvider client={client}>
        <ConversationDetailPage conversationId={id} />
      </QueryClientProvider>
    );
    const view = render(tree(first));
    vi.mocked(uploadConversationFiles).mockResolvedValue({
      files: [{ id: 'uploaded-in-first' }],
    } as never);
    vi.mocked(sendMessage).mockRejectedValueOnce(
      new Error('Failed first message'),
    );
    await user.type(
      screen.getByRole('textbox', { name: 'Message' }),
      'First conversation draft',
    );
    await user.upload(
      document.querySelector('input[type="file"]')!,
      new File(['data'], 'first.txt', { type: 'text/plain' }),
    );
    await user.click(screen.getByRole('button', { name: 'Send' }));
    await screen.findByText(/Failed first message/);
    view.rerender(tree(second));
    expect(screen.getByRole('textbox', { name: 'Message' })).toHaveProperty(
      'value',
      '',
    );
    expect(screen.queryByText('first.txt')).toBeNull();
    expect(screen.queryByText(/Failed first message/)).toBeNull();
    vi.mocked(sendMessage).mockResolvedValue({ id: 'run', status: 'queued' });
    await user.type(
      screen.getByRole('textbox', { name: 'Message' }),
      'Second conversation',
    );
    await user.click(screen.getByRole('button', { name: 'Send' }));
    await waitFor(() =>
      expect(sendMessage).toHaveBeenLastCalledWith(
        second,
        'Second conversation',
        [],
      ),
    );
    expect(uploadConversationFiles).toHaveBeenCalledOnce();
  });
});
