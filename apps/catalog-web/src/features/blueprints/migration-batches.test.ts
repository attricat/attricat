import { describe, expect, it } from 'vitest';
import { hasActiveMigrationForVersion } from './migration-batches';
import type { BlueprintMigrationBatchStatus } from './schemas';

const batch = (
  targetVersion: number,
  status: BlueprintMigrationBatchStatus['status'],
): BlueprintMigrationBatchStatus => ({
  id: '123e4567-e89b-12d3-a456-426614174000',
  blueprint_id: '223e4567-e89b-12d3-a456-426614174000',
  target_version: targetVersion,
  status,
  created_at: '2026-10-05T12:00:00Z',
  started_at: null,
  completed_at: null,
  total_entities: 10,
  processed_entities: 0,
  migrated_entities: 0,
  needs_input_entities: 0,
  failed_entities: 0,
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
