// @vitest-environment jsdom
import { renderHook } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { runStatuses } from '../../api/runStatus';
import type { RuleRun } from './api';
import { useOnRunsFinished } from './runCompletion';

const run = (completedAt: string | null): RuleRun => ({
  id: '00000000-0000-4000-8000-000000000001',
  rule_id: '00000000-0000-4000-8000-000000000002',
  rule_version: 1,
  definition_hash: '',
  source: 'manual',
  dry_run: false,
  scope_record_id: null,
  status: completedAt ? runStatuses.completed : runStatuses.pending,
  candidate_cursor: null,
  candidates_evaluated: 0,
  candidates_skipped: 0,
  findings_created: 0,
  findings_resolved: 0,
  attempts: 0,
  last_error: null,
  truncated: false,
  completed_at: completedAt,
  created_at: '2026-10-28T10:00:00Z',
});

describe('useOnRunsFinished', () => {
  it('reports a run that finishes after the runs first load', () => {
    const onFinished = vi.fn();
    const { rerender } = renderHook(
      ({ runs, loaded }: { runs?: RuleRun[]; loaded: boolean }) =>
        useOnRunsFinished(runs, loaded, onFinished),
      {
        initialProps: { loaded: false } as {
          runs?: RuleRun[];
          loaded: boolean;
        },
      },
    );
    rerender({ runs: [run('2026-10-28T09:00:00Z')], loaded: true });
    expect(onFinished).not.toHaveBeenCalled();
    rerender({
      runs: [run(null), run('2026-10-28T09:00:00Z')],
      loaded: true,
    });
    expect(onFinished).not.toHaveBeenCalled();
    rerender({
      runs: [run('2026-10-28T10:01:00Z'), run('2026-10-28T09:00:00Z')],
      loaded: true,
    });
    expect(onFinished).toHaveBeenCalledTimes(1);
  });
});
