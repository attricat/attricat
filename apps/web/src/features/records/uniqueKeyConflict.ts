import { z } from 'zod';
import { ApiRequestError } from '../../api/request';

/** The API's code when a write would repeat another record's unique value. */
export const uniqueKeyConflictCode = 'unique_key_conflict';

const conflictDetailsSchema = z.object({ conflicting_record_id: z.uuid() });

/** The record that already holds the value `error` reports as taken. */
export const conflictingRecordId = (error: Error | null | undefined) => {
  if (
    !(error instanceof ApiRequestError) ||
    error.code !== uniqueKeyConflictCode
  )
    return undefined;
  return conflictDetailsSchema.safeParse(error.details).data
    ?.conflicting_record_id;
};
