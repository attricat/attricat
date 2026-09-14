// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import '../../i18n';
import type { AgentRun } from './schemas';
import { useConversationLiveUpdates } from './useConversationLiveUpdates';

const sources: TestEventSource[] = [];

class TestEventSource {
  close = vi.fn();
  onerror: ((event: Event) => void) | null = null;
  private readonly listeners = new Map<string, (event: Event) => void>();

  constructor(readonly url: string) {
    sources.push(this);
  }

  addEventListener = (type: string, listener: (event: Event) => void) => {
    this.listeners.set(type, listener);
  };

  emit = (type: string) => this.listeners.get(type)?.(new Event(type));

  fail = () => this.onerror?.(new Event('error'));
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

const LiveUpdatesProbe = ({ runs }: { runs: AgentRun[] }) => {
  const error = useConversationLiveUpdates(runs);
  return error ? <div role="alert">{error}</div> : null;
};

beforeEach(() => {
  sources.length = 0;
  vi.stubGlobal('EventSource', TestEventSource);
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe('useConversationLiveUpdates', () => {
  it('invalidates agent queries for run events and reports disconnects', async () => {
    const queryClient = new QueryClient();
    const invalidateQueries = vi.spyOn(queryClient, 'invalidateQueries');
    const { unmount } = render(
      <QueryClientProvider client={queryClient}>
        <LiveUpdatesProbe runs={[run]} />
      </QueryClientProvider>,
    );

    expect(sources[0].url).toBe(`/api/agent/runs/${run.id}/events`);
    sources[0].emit('message');
    await waitFor(() =>
      expect(invalidateQueries).toHaveBeenCalledWith({
        queryKey: ['agents'],
      }),
    );

    sources[0].fail();
    expect((await screen.findByRole('alert')).textContent).toContain(
      'Live updates disconnected',
    );

    unmount();
    expect(sources[0].close).toHaveBeenCalledOnce();
  });
});
