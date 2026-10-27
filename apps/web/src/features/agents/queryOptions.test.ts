import { QueryClient } from '@tanstack/react-query';
import { describe, expect, it } from 'vitest';
import {
  conversationPollInterval,
  invalidateConversation,
} from './queryOptions';
import { agentQueryKeys } from './queryKeys';
import {
  conversationIdlePollIntervalMs,
  conversationPollIntervalMs,
  conversationReconcileIntervalMs,
} from './constants';
import type { AgentRun } from './schemas';

describe('conversation polling policy', () => {
  const active = [{ status: 'running' }] as AgentRun[];
  it('uses slow polling for idle conversations', () => {
    expect(conversationPollInterval([], false)).toBe(
      conversationIdlePollIntervalMs,
    );
    expect(
      conversationPollInterval([{ status: 'completed' }] as AgentRun[], false),
    ).toBe(conversationIdlePollIntervalMs);
  });
  it('keeps reconciliation while SSE is connected and falls back when disconnected', () => {
    expect(conversationPollInterval(active, true)).toBe(
      conversationReconcileIntervalMs,
    );
    expect(conversationPollInterval(active, false)).toBe(
      conversationPollIntervalMs,
    );
    expect(conversationPollInterval(undefined, false)).toBe(
      conversationPollIntervalMs,
    );
    expect(conversationPollInterval([], false, true)).toBe(
      conversationPollIntervalMs,
    );
  });
});

describe('invalidateConversation', () => {
  it('refreshes the record conversation lists too', async () => {
    const client = new QueryClient();
    const recordList = agentQueryKeys.recordConversations('record-id');
    client.setQueryData(recordList, { pages: [], pageParams: [] });

    await invalidateConversation(client, 'conversation-id');

    expect(client.getQueryState(recordList)?.isInvalidated).toBe(true);
  });
});
