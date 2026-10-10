// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { act, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { agentQueryKeys } from './queryKeys';
import type { AgentRun } from './schemas';
import { useConversationLiveUpdates } from './useConversationLiveUpdates';

const sources: TestEventSource[] = [];
class TestEventSource {
  close = vi.fn();
  onerror: ((event: Event) => void) | null = null;
  onopen: ((event: Event) => void) | null = null;
  private readonly listeners = new Map<string, (event: Event) => void>();
  constructor(readonly url: string) {
    sources.push(this);
  }
  addEventListener = (type: string, listener: (event: Event) => void) => {
    this.listeners.set(type, listener);
  };
  emit = (type: string) => this.listeners.get(type)?.(new Event(type));
  fail = () => this.onerror?.(new Event('error'));
  open = () => this.onopen?.(new Event('open'));
}
const run = {
  conversation_id: '123e4567-e89b-12d3-a456-426614174000',
  created_at: '2026-01-01T00:00:00Z',
  error_code: null,
  error_message: null,
  finished_at: null,
  id: '123e4567-e89b-12d3-a456-426614174001',
  model: 'example',
  origin: 'conversation',
  provider_base_url: 'https://example.test',
  started_at: null,
  status: 'running',
} satisfies AgentRun;
const Probe = ({ runs }: { runs: AgentRun[] }) => {
  const live = useConversationLiveUpdates(run.conversation_id, runs);
  return (
    <>
      <div>{live.connected ? 'Connected' : 'Polling'}</div>
      {live.error && <div role="alert">{live.error}</div>}
    </>
  );
};
beforeEach(() => {
  sources.length = 0;
  vi.stubGlobal('EventSource', TestEventSource);
});
afterEach(() => {
  vi.unstubAllGlobals();
  vi.useRealTimers();
});

describe('useConversationLiveUpdates', () => {
  it('keeps streams stable while run statuses change and recovers on reconnect', async () => {
    const client = new QueryClient();
    const tree = (runs: AgentRun[]) => (
      <QueryClientProvider client={client}>
        <Probe runs={runs} />
      </QueryClientProvider>
    );
    const view = render(tree([run]));
    view.rerender(tree([{ ...run, status: 'awaiting_approval' }]));
    expect(sources).toHaveLength(1);
    expect(sources[0].close).not.toHaveBeenCalled();
    act(() => sources[0].open());
    expect(screen.getByText('Connected')).toBeTruthy();
    act(() => sources[0].fail());
    expect(screen.getByRole('alert').textContent).toContain(
      'Live updates disconnected',
    );
    expect(screen.getByText('Polling')).toBeTruthy();
    act(() => sources[0].open());
    expect(screen.queryByRole('alert')).toBeNull();
    expect(screen.getByText('Connected')).toBeTruthy();
    view.rerender(tree([{ ...run, status: 'completed' }]));
    expect(sources[0].close).toHaveBeenCalledOnce();
    expect(screen.queryByRole('alert')).toBeNull();
  });

  it('coalesces event bursts and invalidates only the affected conversation', async () => {
    const client = new QueryClient();
    const invalidate = vi.spyOn(client, 'invalidateQueries');
    const unrelated = agentQueryKeys.messages('other-conversation');
    client.setQueryData(unrelated, []);
    render(
      <QueryClientProvider client={client}>
        <Probe runs={[run]} />
      </QueryClientProvider>,
    );
    expect(sources[0].url).toBe(`/api/agent/runs/${run.id}/events`);
    for (const event of ['message', 'status', 'tool_call', 'terminal'])
      sources[0].emit(event);
    await waitFor(() => expect(invalidate).toHaveBeenCalledTimes(2));
    expect(invalidate).toHaveBeenCalledWith({
      queryKey: agentQueryKeys.conversation(run.conversation_id),
    });
    expect(invalidate).toHaveBeenCalledWith({
      queryKey: agentQueryKeys.approvals(run.conversation_id),
    });
    expect(client.getQueryState(unrelated)?.isInvalidated).toBe(false);
  });

  it('disposes pending refresh timers and ignores late events', async () => {
    vi.useFakeTimers();
    const client = new QueryClient();
    const invalidate = vi.spyOn(client, 'invalidateQueries');
    const view = render(
      <QueryClientProvider client={client}>
        <Probe runs={[run]} />
      </QueryClientProvider>,
    );
    sources[0].emit('message');
    view.unmount();
    sources[0].emit('terminal');
    await vi.runAllTimersAsync();
    expect(invalidate).not.toHaveBeenCalled();
    expect(sources[0].close).toHaveBeenCalledOnce();
  });
});
