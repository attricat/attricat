// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { decideApproval, listApprovals, listMessages, listRuns } from './api';
import { ConversationPanel } from './ConversationPanel';
import { httpStatus } from '../../api/httpStatus';
import { ApiRequestError } from '../../api/request';

vi.mock('./api', async (importOriginal) => ({
  ...(await importOriginal<typeof import('./api')>()),
  decideApproval: vi.fn(),
  listApprovals: vi.fn(),
  listMessages: vi.fn(),
  listRuns: vi.fn(),
}));
vi.mock('./useConversationLiveUpdates', () => ({
  useConversationLiveUpdates: () => ({ connected: true }),
}));
vi.mock('./ConversationComposer', () => ({ ConversationComposer: () => null }));
vi.mock('./ConversationTranscript', () => ({
  ConversationTranscript: ({
    onDecideApproval,
  }: {
    onDecideApproval: (id: string, approved: boolean) => void;
  }) => <button onClick={() => onDecideApproval('call', true)}>Approve</button>,
}));

const renderPanel = () =>
  render(
    <QueryClientProvider
      client={
        new QueryClient({ defaultOptions: { queries: { retry: false } } })
      }
    >
      <ConversationPanel conversationId="123e4567-e89b-12d3-a456-426614174000" />
    </QueryClientProvider>,
  );

describe('ConversationPanel', () => {
  beforeEach(() => {
    vi.mocked(listApprovals).mockReset().mockResolvedValue([]);
    vi.mocked(listMessages).mockResolvedValue([]);
    vi.mocked(listRuns).mockResolvedValue([]);
  });

  it('reports a refused approval and refreshes the approvals', async () => {
    vi.mocked(decideApproval).mockRejectedValue(
      new ApiRequestError(httpStatus.conflict, 'refused'),
    );
    const user = userEvent.setup();
    renderPanel();
    await waitFor(() => expect(listApprovals).toHaveBeenCalledTimes(1));
    expect(screen.queryByRole('alert')).toBeNull();

    await user.click(screen.getByRole('button', { name: 'Approve' }));

    expect(await screen.findByRole('alert')).toBeTruthy();
    await waitFor(() => expect(listApprovals).toHaveBeenCalledTimes(2));
  });
});
