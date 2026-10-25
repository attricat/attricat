import { z } from 'zod';
import { request } from '../../api/request';
import {
  createReusableAttributeGroupSchema,
  createReusableAttributeSchema,
  reusableAttributeGroupSchema,
  reusableAttributeSchema,
  type CreateReusableAttribute,
  type ReusableAttribute,
  type ReusableAttributeGroup,
} from './schemas';
import { uuidSegment } from '../../api/pathSegments';

export type { ReusableAttribute, ReusableAttributeGroup } from './schemas';

export const listReusableAttributes = (
  includeDrafts = false,
  signal?: AbortSignal,
) =>
  request(
    `/api/reusable-attributes${includeDrafts ? '?include_drafts=true' : ''}`,
    z.array(reusableAttributeSchema),
    { signal },
  );

export const createReusableAttribute = (input: CreateReusableAttribute) =>
  request('/api/reusable-attributes', reusableAttributeSchema, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(createReusableAttributeSchema.parse(input)),
  });

export const createReusableAttributeRevision = (
  definitionId: string,
  input: CreateReusableAttribute,
) =>
  request(
    `/api/reusable-attributes/${uuidSegment(definitionId)}/versions`,
    reusableAttributeSchema,
    {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(createReusableAttributeSchema.parse(input)),
    },
  );

/**
 * Runs every check that saving would, without storing anything. With an id it
 * checks the next revision of that reusable attribute.
 */
export const validateReusableAttribute = (
  input: CreateReusableAttribute,
  definitionId?: string,
  signal?: AbortSignal,
) =>
  request(
    definitionId === undefined
      ? '/api/reusable-attributes/validate'
      : `/api/reusable-attributes/${uuidSegment(definitionId)}/versions/validate`,
    reusableAttributeSchema,
    {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(createReusableAttributeSchema.parse(input)),
      signal,
    },
  );

export const publishReusableAttributeRevision = (
  revisionId: string,
): Promise<ReusableAttribute> =>
  request(
    `/api/reusable-attribute-revisions/${uuidSegment(revisionId)}/publish`,
    reusableAttributeSchema,
    { method: 'POST' },
  );

export const listReusableAttributeGroups = (signal?: AbortSignal) =>
  request(
    '/api/reusable-attribute-groups',
    z.array(reusableAttributeGroupSchema),
    { signal },
  );

export const createReusableAttributeGroup = (input: {
  code: string;
  name: string;
  position: number;
  reusable_attribute_revision_ids: string[];
}): Promise<ReusableAttributeGroup> =>
  request('/api/reusable-attribute-groups', reusableAttributeGroupSchema, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(createReusableAttributeGroupSchema.parse(input)),
  });

export const attachReusableAttribute = (recordId: string, revisionId: string) =>
  request(
    `/api/v1/records/${uuidSegment(recordId)}/reusable-attributes`,
    z.unknown(),
    {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        reusable_attribute_revision_id: z.uuid().parse(revisionId),
      }),
    },
  );

export const attachReusableAttributeGroup = (
  recordId: string,
  groupId: string,
) =>
  request(
    `/api/v1/records/${uuidSegment(recordId)}/reusable-attribute-groups/${uuidSegment(groupId)}`,
    z.array(z.unknown()),
    { method: 'POST' },
  );
