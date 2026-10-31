import { describe, expect, it } from 'vitest';
import { ApiRequestError } from '../../api/request';
import { conflictingRecordId } from './uniqueKeyConflict';

describe('conflictingRecordId', () => {
  const id = '00000000-0000-4000-8000-000000000001';

  it('reads the conflicting record of a unique key conflict', () => {
    expect(
      conflictingRecordId(
        new ApiRequestError(409, 'taken', 'unique_key_conflict', {
          conflicting_record_id: id,
        }),
      ),
    ).toBe(id);
  });

  it('ignores other errors', () => {
    expect(
      conflictingRecordId(
        new ApiRequestError(409, 'x', 'conflict', {
          conflicting_record_id: id,
        }),
      ),
    ).toBeUndefined();
    expect(conflictingRecordId(new Error('x'))).toBeUndefined();
  });
});
