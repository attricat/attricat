import { z } from 'zod';

/**
 * The lifecycle shared by blueprint, reusable attribute, rule and workflow
 * revisions: a revision is drafted, then published.
 */
export const revisionStatuses = {
  draft: 'draft',
  published: 'published',
} as const;

export const revisionStatusSchema = z.enum([
  revisionStatuses.draft,
  revisionStatuses.published,
]);

export type RevisionStatus = z.infer<typeof revisionStatusSchema>;
