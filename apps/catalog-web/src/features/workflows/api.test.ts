import { afterEach, describe, expect, it, vi } from 'vitest';
import { listWorkflowRuns, runWorkflowNow, validateWorkflow } from './api';

const fetchMock = vi.fn();
vi.stubGlobal('fetch', fetchMock);
vi.stubGlobal('document', { cookie: '' });

afterEach(() => fetchMock.mockReset());

const run = {
  id: '123e4567-e89b-12d3-a456-426614174000',
  workflow_id: '223e4567-e89b-12d3-a456-426614174000',
  workflow_version: 1,
  trigger_event_id: '323e4567-e89b-12d3-a456-426614174000',
  trigger_sequence: 4,
  source: 'event',
  status: 'dead_letter',
  attempts: 5,
  failed_at: '2026-01-01T00:00:00Z',
  completed_at: null,
  last_error: 'A safe error',
  created_at: '2026-01-01T00:00:00Z',
  cancelled_at: null,
  root_trigger_event_id: '423e4567-e89b-12d3-a456-426614174000',
  causal_depth: 0,
};

describe('workflow API client', () => {
  it('reads only the safe workflow run diagnostic contract', async () => {
    fetchMock.mockResolvedValue({
      ok: true,
      json: () =>
        Promise.resolve([
          { ...run, trigger_event_payload: { secret: 'never exposed' } },
        ]),
    });

    await expect(listWorkflowRuns()).resolves.toEqual([run]);
    expect(fetchMock).toHaveBeenCalledWith('/api/workflow-runs');
  });

  it('starts a bounded manual run with only a record target', async () => {
    fetchMock.mockResolvedValue({
      ok: true,
      json: () => Promise.resolve({ id: run.id }),
    });

    await expect(
      runWorkflowNow(run.workflow_id, run.trigger_event_id, 'manual-test-key'),
    ).resolves.toEqual({ id: run.id });
    expect(fetchMock).toHaveBeenCalledWith(
      `/api/workflows/${run.workflow_id}/run-now`,
      {
        body: JSON.stringify({
          record_id: run.trigger_event_id,
          idempotency_key: 'manual-test-key',
        }),
        headers: { 'Content-Type': 'application/json' },
        method: 'POST',
      },
    );
  });

  it('sends TOML to the server compiler for canonical validation', async () => {
    fetchMock.mockResolvedValue({
      ok: true,
      json: () =>
        Promise.resolve({
          code: 'example-workflow',
          name: 'Example workflow',
          format_version: 1,
          triggers: [],
          actions: [],
          raw_definition_hash: 'a'.repeat(64),
        }),
    });

    await expect(validateWorkflow('format_version = 1')).resolves.toMatchObject(
      {
        code: 'example-workflow',
      },
    );
    expect(fetchMock).toHaveBeenCalledWith('/api/workflows/validate', {
      body: JSON.stringify({ definition: 'format_version = 1' }),
      headers: { 'Content-Type': 'application/json' },
      method: 'POST',
    });
  });
});
