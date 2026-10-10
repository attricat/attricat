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

export type {
  CreateReusableAttribute,
  ReusableAttribute,
  ReusableAttributeGroup,
} from './schemas';

export const listReusableAttributes = (
  includeDrafts = false,
  signal?: AbortSignal,
) =>
  request(
    `/api/reusable-attributes${includeDrafts ? '?include_drafts=true' : ''}`,
    z.array(reusableAttributeSchema),
    signal === undefined ? undefined : { signal },
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
    `/api/reusable-attributes/${encodeURIComponent(z.uuid().parse(definitionId))}/versions`,
    reusableAttributeSchema,
    {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(createReusableAttributeSchema.parse(input)),
    },
  );

export const publishReusableAttributeRevision = (
  revisionId: string,
): Promise<ReusableAttribute> =>
  request(
    `/api/reusable-attribute-revisions/${encodeURIComponent(z.uuid().parse(revisionId))}/publish`,
    reusableAttributeSchema,
    { method: 'POST' },
  );

export const listReusableAttributeGroups = (signal?: AbortSignal) =>
  request(
    '/api/reusable-attribute-groups',
    z.array(reusableAttributeGroupSchema),
    signal === undefined ? undefined : { signal },
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
    `/api/v1/records/${encodeURIComponent(z.uuid().parse(recordId))}/reusable-attributes`,
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
    `/api/v1/records/${encodeURIComponent(z.uuid().parse(recordId))}/reusable-attribute-groups/${encodeURIComponent(z.uuid().parse(groupId))}`,
    z.array(z.unknown()),
    { method: 'POST' },
  );
