// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { searchConversations } from './api';
import { RecordConversationsPanel } from './RecordConversationsPanel';

vi.mock('./api', () => ({ searchConversations: vi.fn() }));
vi.mock('../principals/PrincipalValue', () => ({
  PrincipalText: ({ value }: { value: string }) => <>{value}</>,
}));
vi.mock('../../components/RouterLink', () => ({
  RouterIconButton: ({
    children,
    params,
    'aria-label': label,
  }: {
    children: React.ReactNode;
    params: { conversationId: string };
    'aria-label': string;
  }) => (
    <a aria-label={label} href={`/agents/${params.conversationId}`}>
      {children}
    </a>
  ),
}));

const recordId = '123e4567-e89b-12d3-a456-426614174000';
const contextId = '123e4567-e89b-12d3-a456-426614174001';
const userId = '123e4567-e89b-12d3-a456-426614174004';
const conversation = (id: string, title: string, context: string | null) => ({
  id,
  workspace_id: '123e4567-e89b-12d3-a456-426614174009',
  created_by_user_id: userId,
  title,
  title_source: 'generated',
  record_id: recordId,
  context_id: context,
  created_at: '2026-10-26T00:00:00Z',
  updated_at: '2026-10-26T00:00:00Z',
  archived_at: null,
});

const renderPanel = (onOpen = vi.fn(), onStart = vi.fn()) => {
  render(
    <QueryClientProvider
      client={
        new QueryClient({ defaultOptions: { queries: { retry: false } } })
      }
    >
      <RecordConversationsPanel
        contextCodes={new Map([[contextId, 'pl']])}
        onOpen={onOpen}
        onStart={onStart}
        recordId={recordId}
      />
    </QueryClientProvider>,
  );
  return { onOpen, onStart };
};

describe('RecordConversationsPanel', () => {
  beforeEach(() => vi.mocked(searchConversations).mockReset());

  it("lists the record's conversations and opens one beside the record", async () => {
    vi.mocked(searchConversations).mockResolvedValue({
      items: [
        conversation('one', 'Polish copy', contextId),
        conversation('two', 'Missing sizes', null),
      ] as never,
      next_cursor: null,
    });
    const user = userEvent.setup();
    const { onOpen } = renderPanel();
    expect(await screen.findByText('Polish copy')).toBeTruthy();
    expect(searchConversations).toHaveBeenCalledWith('', undefined, {
      recordId,
    });
    expect(screen.getByText('pl')).toBeTruthy();
    expect(
      screen.getAllByText(`user:${userId}`, { exact: false }),
    ).toHaveLength(2);
    expect(
      screen
        .getAllByRole('link', { name: 'Open on the Agents page' })[1]
        .getAttribute('href'),
    ).toBe('/agents/two');
    await user.click(screen.getByText('Missing sizes'));
    expect(onOpen).toHaveBeenCalledWith('two');
  });

  it('starts the first conversation from its empty state', async () => {
    vi.mocked(searchConversations).mockResolvedValue({
      items: [],
      next_cursor: null,
    });
    const user = userEvent.setup();
    const { onStart } = renderPanel();
    expect(await screen.findByText('No conversations yet')).toBeTruthy();
    await user.click(screen.getByRole('button', { name: 'New conversation' }));
    expect(onStart).toHaveBeenCalledOnce();
  });
});
