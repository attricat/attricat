import { useInfiniteQuery } from '@tanstack/react-query';
import {
  recordApprovalsOptions,
  recordRetentionHoldsOptions,
} from './queryOptions';

/**
 * Whether the record has approvals or retention holds to show, or a failure
 * reading them. Shares the panel's first pages.
 */
export const useHasRecordControls = (recordId: string) => {
  const approvals = useInfiniteQuery(recordApprovalsOptions(recordId));
  const holds = useInfiniteQuery(recordRetentionHoldsOptions(recordId));
  return (
    approvals.isError ||
    holds.isError ||
    (approvals.data?.pages[0]?.items.length ?? 0) > 0 ||
    (holds.data?.pages[0]?.items.length ?? 0) > 0
  );
};
