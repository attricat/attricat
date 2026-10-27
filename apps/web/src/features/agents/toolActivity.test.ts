import { describe, expect, it } from 'vitest';
import { parseDraftProposal } from './parseDraftProposal';
import type { ConversationMessage } from './schemas';
import { groupToolActivity } from './toolActivity';

const message = (
  id: string,
  role: string,
  content: unknown,
): ConversationMessage => ({
  id,
  conversation_id: 'conversation',
  run_id: null,
  sequence: 0,
  role,
  content,
  created_at: '2026-10-27T00:00:00Z',
  attachments: [],
});

describe('groupToolActivity', () => {
  it('gathers consecutive tool calls and results between replies', () => {
    const question = message('1', 'user', 'How many?');
    const call = message('2', 'assistant', {
      tool_calls: [{ id: 'a', function: { name: 'count_records' } }],
    });
    const result = message('3', 'tool', { tool_call_id: 'a', result: 3 });
    const nextCall = message('4', 'assistant', {
      tool_calls: [{ id: 'b', function: { name: 'search_records' } }],
    });
    const reply = message('5', 'assistant', 'Three.');

    expect(
      groupToolActivity([question, call, result, nextCall, reply]),
    ).toEqual([question, [call, result, nextCall], reply]);
  });
});

describe('parseDraftProposal', () => {
  it('treats a proposal without values as a plain reply', () => {
    expect(
      parseDraftProposal({
        draft_proposal: { fields: {}, explanation: 'It is a car.' },
      }),
    ).toBeNull();
  });
});
