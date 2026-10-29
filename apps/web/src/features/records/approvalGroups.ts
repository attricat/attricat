import { defaultContextCode } from '../contexts/constants';
import type { RecordApproval } from './api';

/** One context's copy of a grouped approval. */
export type ApprovalGroupContext = {
  id: string;
  code: string;
  digest: string;
};

/** Approvals one status change recorded in several contexts. */
export type ApprovalGroup = {
  /** The newest approval; every member shares its decision fields. */
  approval: RecordApproval;
  contexts: ApprovalGroupContext[];
};

/** What makes approvals the same decision: everything but their context. */
const decisionKey = (approval: RecordApproval) =>
  JSON.stringify([
    approval.attribute_code,
    approval.status,
    approval.approved_at,
    approval.approved_by_user_id,
    approval.approved_by,
    approval.covers_all,
    approval.covered_attributes,
    approval.end_reason,
    approval.ended_at,
    approval.void_status,
    approval.cause,
  ]);

/** The default context first, then the others by code. */
const byContext = (left: ApprovalGroupContext, right: ApprovalGroupContext) =>
  Number(right.code === defaultContextCode) -
    Number(left.code === defaultContextCode) ||
  left.code.localeCompare(right.code);

/**
 * Groups approvals that share their status, time, approver, end and cause,
 * newest first, each listing its contexts in order. Pages never split one
 * approval time, so a loaded group is complete.
 */
export const groupApprovals = (
  approvals: readonly RecordApproval[],
): ApprovalGroup[] => {
  const groups = new Map<string, ApprovalGroup>();
  for (const approval of approvals) {
    const key = decisionKey(approval);
    const context = {
      id: approval.id,
      code: approval.context_code,
      digest: approval.content_digest,
    };
    const group = groups.get(key);
    if (group) group.contexts.push(context);
    else groups.set(key, { approval, contexts: [context] });
  }
  const ordered = [...groups.values()];
  for (const group of ordered) group.contexts.sort(byContext);
  return ordered;
};
