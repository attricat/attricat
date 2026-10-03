// @vitest-environment jsdom
import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import '../../i18n';
import {
  ExtensionRunProgress,
  ExtensionRunStatusChip,
} from './ExtensionRunSummary';
import type { ExtensionRun } from './schemas';

const run = (overrides: Partial<ExtensionRun>): ExtensionRun => ({
  id: '11111111-1111-4111-8111-111111111111',
  extension_id: 'acme.docs',
  contribution_id: 'dialog',
  operation_id: 'generate',
  status: 'completed',
  progress: {},
  failure: null,
  can_cancel: false,
  selection_count: 4,
  blueprint_id: null,
  blueprint_version: null,
  context_id: null,
  created_at: '2026-10-02T12:00:00Z',
  completed_at: '2026-10-02T12:01:00Z',
  cancelled_at: null,
  outputs_expire_at: '2026-11-01T12:01:00Z',
  outputs_expired: false,
  ...overrides,
});

describe('extension run summary', () => {
  it('reports the domain outcome separately from the execution status', () => {
    render(
      <>
        <ExtensionRunStatusChip status="completed" />
        <ExtensionRunProgress
          run={run({
            progress: {
              completed: 4,
              total: 4,
              outcome: { succeeded: 3, failed: 1, skipped: 0 },
            },
          })}
        />
      </>,
    );
    expect(screen.getByText('Completed')).toBeTruthy();
    expect(screen.getByText('4 of 4 processed')).toBeTruthy();
    expect(screen.getByText('3 succeeded · 1 failed · 0 skipped')).toBeTruthy();
  });

  it('ignores progress fields it does not understand', () => {
    render(
      <ExtensionRunProgress
        run={run({ status: 'running', progress: { phase: 'render' } })}
      />,
    );
    expect(screen.getByRole('progressbar')).toBeTruthy();
    expect(screen.queryByText(/processed/)).toBeNull();
  });
});
