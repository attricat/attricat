import { describe, expect, it } from 'vitest';
import { hasActiveMigrationForVersion } from './migrationBatches';
import type { BlueprintMigrationBatchStatus } from './schemas';

const batch = (
  targetVersion: number,
  status: BlueprintMigrationBatchStatus['status'],
): BlueprintMigrationBatchStatus => ({
  id: '123e4567-e89b-12d3-a456-426614174000',
  blueprint_id: '223e4567-e89b-12d3-a456-426614174000',
  target_version: targetVersion,
  removal_policy: {},
  status,
  created_at: '2026-10-05T12:00:00Z',
  started_at: null,
  completed_at: null,
  total_records: 10,
  processed_records: 0,
  migrated_records: 0,
  needs_input_records: 0,
  failed_records: 0,
});

describe('hasActiveMigrationForVersion', () => {
  it('matches only queued or running batches for the requested version', () => {
    expect(
      hasActiveMigrationForVersion(
        [batch(2, 'completed'), batch(3, 'running')],
        3,
      ),
    ).toBe(true);
    expect(
      hasActiveMigrationForVersion(
        [batch(2, 'running'), batch(3, 'completed')],
        3,
      ),
    ).toBe(false);
  });
});
