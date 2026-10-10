import { describe, expect, it } from 'vitest';
import { conversationPollInterval } from './queryOptions';
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
