import { z } from 'zod';
import { request } from '../../api/request';
import { uuidSchema } from './schemas';

const statusTransitionAccessSchema = z.object({
  attribute_code: z.string(),
  from: z.string().nullable(),
  to: z.string().nullable(),
  code: z.string().nullable(),
  allowed: z.boolean(),
  denial_code: z
    .enum(['status_transition_forbidden', 'status_separation_of_duties'])
    .nullable(),
  denial_reason: z.string().nullable(),
});
export type StatusTransitionAccess = z.infer<
  typeof statusTransitionAccessSchema
>;

const entityApprovalSchema = z.object({
  id: z.string(),
  attribute_code: z.string(),
  context_code: z.string(),
  status: z.string(),
  covers_all: z.boolean(),
  covered_attributes: z.array(z.string()),
  content_digest: z.string(),
  approved_by: z.string().nullable(),
  approved_at: z.string(),
  ended_at: z.string().nullable(),
  end_reason: z.enum(['content_changed', 'superseded']).nullable(),
  void_status: z.string().nullable(),
});
export type EntityApproval = z.infer<typeof entityApprovalSchema>;

const retentionHoldSchema = z.object({
  id: z.string(),
  file_id: z.string(),
  source: z.enum(['status', 'explicit']),
  attribute_code: z.string().nullable(),
  status: z.string().nullable(),
  reason: z.string().nullable(),
  held_until: z.string(),
  released_at: z.string().nullable(),
  active: z.boolean(),
});
export type RetentionHold = z.infer<typeof retentionHoldSchema>;

const items = <T extends z.ZodType>(item: T) =>
  z.object({ items: z.array(item) }).transform(({ items }) => items);

const entityPath = (id: string) =>
  `/api/v1/entities/${encodeURIComponent(uuidSchema.parse(id))}`;

/** Declared edges from the saved status and whether the caller may take them. */
export const getStatusTransitions = (
  id: string,
  contextId: string | null,
  signal?: AbortSignal,
) =>
  request(
    `${entityPath(id)}/status-transitions${
      contextId
        ? `?context_id=${encodeURIComponent(uuidSchema.parse(contextId))}`
        : ''
    }`,
    items(statusTransitionAccessSchema),
    signal === undefined ? undefined : { signal },
  );

export const getEntityApprovals = (id: string, signal?: AbortSignal) =>
  request(
    `${entityPath(id)}/approvals`,
    items(entityApprovalSchema),
    signal === undefined ? undefined : { signal },
  );

export const getEntityRetentionHolds = (id: string, signal?: AbortSignal) =>
  request(
    `${entityPath(id)}/retention-holds`,
    items(retentionHoldSchema),
    signal === undefined ? undefined : { signal },
  );
