import { describe, expect, it } from 'vitest';
import type { RecordApproval } from './api';
import { groupApprovals } from './approvalGroups';

const approval = (
  id: string,
  context_code: string,
  overrides: Partial<RecordApproval> = {},
): RecordApproval => ({
  id,
  attribute_code: 'status',
  context_code,
  status: 'approved',
  covers_all: true,
  covered_attributes: [],
  content_digest: 'a'.repeat(64),
  approved_by_user_id: null,
  approved_by: 'Ada',
  approved_at: '2026-10-20T10:00:00Z',
  ended_at: null,
  end_reason: null,
  void_status: null,
  cause: null,
  ...overrides,
});

describe('groupApprovals', () => {
  it('groups one decision recorded in several contexts, newest first', () => {
    const groups = groupApprovals([
      approval('2', 'cars_pl', { content_digest: 'b'.repeat(64) }),
      approval('5', 'cars_de'),
      approval('1', 'default'),
      // Another end is another decision, even at the same time.
      approval('3', 'cars_de', {
        end_reason: 'superseded',
        ended_at: '2026-10-21T10:00:00Z',
      }),
      approval('4', 'default', { approved_at: '2026-10-19T10:00:00Z' }),
    ]);
    expect(
      groups.map((group) => group.contexts.map((context) => context.code)),
    ).toEqual([['default', 'cars_de', 'cars_pl'], ['cars_de'], ['default']]);
    expect(groups[0].contexts[2].digest).toBe('b'.repeat(64));
  });
});
