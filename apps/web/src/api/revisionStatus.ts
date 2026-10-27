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

/**
 * The newest revision of each resource, in the order each resource first
 * appears; `idOf` names the resource a revision belongs to.
 */
export const latestRevisions = <T extends { version: number }>(
  revisions: readonly T[],
  idOf: (revision: T) => string,
): T[] => {
  const latest = new Map<string, T>();
  for (const revision of revisions) {
    const known = latest.get(idOf(revision));
    if (!known || revision.version > known.version)
      latest.set(idOf(revision), revision);
  }
  return [...latest.values()];
};
