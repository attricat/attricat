import { z } from 'zod';
import { request, requestNoContent } from '../../api/request';
import {
  RECORD_CHANGES_PAGE_SIZE,
  RECORD_PREVIEW_QUERY,
  RECORD_SEARCH_PAGE_SIZE,
} from './constants';
import {
  blueprintSchema,
  blueprintWithAttributesSchema,
  recordLabelsResponseSchema,
  createRecordRequestSchema,
  recordHierarchySchema,
  recordAuditChangeSchema,
  recordFormResponseSchema,
  recordMigrationPreviewSchema,
  recordPreviewResponseSchema,
  recordSchema,
  recordSearchResponseSchema,
  getBlueprintRequestSchema,
  getBlueprintRevisionRequestSchema,
  incomingRelationshipsPageSchema,
  resolvedRecordPreviewSchema,
  smartFillRecordFormRequestSchema,
  smartFillRecordFormResponseSchema,
  searchRecordsRequestSchema,
  migrateRecordRequestSchema,
  updateRecordRequestSchema,
  recordPublicationStatusSchema,
  recordPublicationReadinessSchema,
  recordApprovalSchema,
  itemsResponseSchema,
  retentionHoldSchema,
  statusTransitionAccessSchema,
  uuidSchema,
} from './schemas';

export { viewBlockTypes } from './schemas';
export type {
  Attribute,
  Blueprint,
  BlueprintWithAttributes,
  CheckViolation,
  ComponentReference,
  CatalogRecord,
  RecordApproval,
  RecordAuditChange,
  RecordFormResponse,
  RecordMigrationPreview,
  RecordPublicationReadiness,
  RecordPublicationStatus,
  FormAttributeValue,
  RecordItem,
  RecordSearchFilter,
  RecordSearchResponse,
  JsonSchema,
  NewAttributeValue,
  NewFileAttributeValue,
  RelationshipTargets,
  ResolvedRecordPreview,
  RetentionHold,
  StatusTransitionAccess,
  ViewDefinition,
  ViewNode,
} from './schemas';

export { ApiRequestError } from '../../api/request';

/** Path of one record in the versioned record API. */
const recordPath = (id: string) =>
  `/api/v1/records/${encodeURIComponent(uuidSchema.parse(id))}`;

export type SearchRecordsOptions = {
  blueprint: string;
  /** Context whose values filters, sorting and table values resolve. */
  contextCode?: string;
  cursor?: string | null;
  filters?: import('./schemas').RecordSearchFilter[];
  includeTotal?: boolean;
  query?: string;
  relationshipFilters?: {
    field: string;
    selected_target_ids: string[];
  }[];
  signal?: AbortSignal;
  sort?: { field: string; direction: 'asc' | 'desc'; context_code?: string };
  version?: number;
};

export const searchRecords = ({
  blueprint,
  contextCode,
  cursor = null,
  filters = [],
  includeTotal = false,
  query = '',
  relationshipFilters = [],
  signal,
  sort,
  version,
}: SearchRecordsOptions) => {
  const payload = searchRecordsRequestSchema.parse({
    blueprint: {
      code: blueprint,
      ...(version === undefined ? {} : { version }),
    },
    query,
    filters,
    ...(relationshipFilters.length
      ? { relationship_filters: relationshipFilters }
      : {}),
    sort,
    ...(contextCode === undefined ? {} : { context_code: contextCode }),
    ...(includeTotal ? { include_total: true } : {}),
    page: { size: RECORD_SEARCH_PAGE_SIZE, cursor },
  });
  return request('/api/v1/records/search', recordSearchResponseSchema, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(payload),
    ...(signal === undefined ? {} : { signal }),
  });
};

export const listRecordBlueprints = (signal?: AbortSignal) =>
  request(
    '/api/blueprints',
    z.array(blueprintSchema),
    signal === undefined ? undefined : { signal },
  );
export const getRecordLabels = (
  recordIds: readonly string[],
  signal?: AbortSignal,
) =>
  request('/api/v1/records/labels', recordLabelsResponseSchema, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ record_ids: z.array(uuidSchema).parse(recordIds) }),
    ...(signal === undefined ? {} : { signal }),
  });
export const getRecordPreview = (id: string, signal?: AbortSignal) =>
  request(
    `/api/records/${encodeURIComponent(uuidSchema.parse(id))}/preview?relationship_depth=${RECORD_PREVIEW_QUERY.relationshipDepth}&relationship_limit=${RECORD_PREVIEW_QUERY.relationshipLimit}`,
    recordPreviewResponseSchema,
    signal === undefined ? undefined : { signal },
  );
export const getResolvedRecordPreview = (id: string, contextId: string) =>
  request(
    `/api/records/${encodeURIComponent(uuidSchema.parse(id))}/resolved-preview?context_id=${encodeURIComponent(uuidSchema.parse(contextId))}`,
    resolvedRecordPreviewSchema,
  );
export const getRecordHierarchy = (
  id: string,
  contextId: string,
  field: string,
) =>
  request(
    `/api/records/${encodeURIComponent(uuidSchema.parse(id))}/hierarchy?context_id=${encodeURIComponent(uuidSchema.parse(contextId))}&field=${encodeURIComponent(field)}`,
    recordHierarchySchema,
  );
export const getBlueprintByCode = (
  code: string,
  version?: number,
  signal?: AbortSignal,
) => {
  const input = getBlueprintRequestSchema.parse({ code, version });
  const path = `/api/blueprints/by-code/${encodeURIComponent(input.code)}${input.version === undefined ? '' : `/versions/${input.version}`}`;
  return request(
    path,
    blueprintWithAttributesSchema,
    signal === undefined ? undefined : { signal },
  );
};
export const getBlueprintRevision = (id: string, version: number) => {
  const input = getBlueprintRevisionRequestSchema.parse({ id, version });
  return request(
    `/api/blueprints/${encodeURIComponent(input.id)}/versions/${input.version}`,
    blueprintWithAttributesSchema,
  );
};
export const getCurrentBlueprint = (id: string) =>
  request(
    `/api/blueprints/${encodeURIComponent(uuidSchema.parse(id))}`,
    blueprintWithAttributesSchema,
  );
export const getRecordChanges = (id: string, offset: number) =>
  request(
    `/api/records/${encodeURIComponent(uuidSchema.parse(id))}/changes?limit=${RECORD_CHANGES_PAGE_SIZE}&offset=${offset}`,
    z.object({
      items: z.array(recordAuditChangeSchema),
      next_offset: z.number().int().nonnegative().nullable(),
    }),
  );
export const getRecordForm = (id: string, signal?: AbortSignal) =>
  request(
    recordPath(id),
    recordFormResponseSchema,
    signal === undefined ? undefined : { signal },
  );
export const getIncomingRelationships = (
  id: string,
  relationships: { source_blueprint: string; field: string }[],
  pageSize: number,
  cursor: string | null,
) =>
  request(
    `${recordPath(id)}/incoming-relationships`,
    incomingRelationshipsPageSchema,
    {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        relationships,
        page: { size: pageSize, cursor },
      }),
    },
  );
export const createRecord = (
  input: z.input<typeof createRecordRequestSchema>,
) => {
  const payload = createRecordRequestSchema.parse(input);
  return request('/api/v1/records', recordSchema, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(payload),
  });
};
export const smartFillRecordForm = (
  input: z.input<typeof smartFillRecordFormRequestSchema>,
) => {
  const payload = smartFillRecordFormRequestSchema.parse(input);
  return request('/api/agent/smart-fill', smartFillRecordFormResponseSchema, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(payload),
  });
};

export const deleteRecord = (id: string) =>
  requestNoContent(`/api/records/${encodeURIComponent(uuidSchema.parse(id))}`, {
    method: 'DELETE',
  });

export const duplicateRecord = (id: string) =>
  request(`${recordPath(id)}/duplicate`, recordSchema, { method: 'POST' });

export const updateRecord = (
  id: string,
  input: z.input<typeof updateRecordRequestSchema>,
) => {
  const path = recordPath(id);
  const payload = updateRecordRequestSchema.parse(input);
  return request(path, recordSchema, {
    method: 'PUT',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(payload),
  });
};

export const getRecordPublications = (id: string, signal?: AbortSignal) =>
  request(
    `${recordPath(id)}/publications`,
    z.array(recordPublicationStatusSchema),
    signal === undefined ? undefined : { signal },
  );
/** Whether the record passes each enabled channel's publication checks. */
export const getRecordPublicationReadiness = (
  id: string,
  signal?: AbortSignal,
) =>
  request(
    `${recordPath(id)}/publications/readiness`,
    z.array(recordPublicationReadinessSchema),
    signal === undefined ? undefined : { signal },
  );
export const publishRecord = (id: string, contextId: string) =>
  request(`${recordPath(id)}/publications`, recordPublicationStatusSchema, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ context_id: uuidSchema.parse(contextId) }),
  });
export const publishRecordAllChannels = (id: string) =>
  request(
    `${recordPath(id)}/publications/publish-all`,
    z.array(recordPublicationStatusSchema),
    { method: 'POST' },
  );
export const unpublishRecord = (id: string, contextId: string) =>
  requestNoContent(`${recordPath(id)}/publications/unpublish`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ context_id: uuidSchema.parse(contextId) }),
  });
export const previewRecordMigration = (id: string) =>
  request(
    `${recordPath(id)}/blueprint-migration/preview`,
    recordMigrationPreviewSchema,
    { method: 'POST' },
  );

export const migrateRecord = (
  id: string,
  input: z.input<typeof migrateRecordRequestSchema>,
) => {
  const path = recordPath(id);
  const { expected_updated_at, ...payload } =
    migrateRecordRequestSchema.parse(input);
  const query = expected_updated_at
    ? `?${new URLSearchParams({ expected_updated_at })}`
    : '';
  return request(`${path}/blueprint-migration${query}`, recordSchema, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(payload),
  });
};

/** Declared edges from the saved status and whether the caller may take them. */
export const getStatusTransitions = (
  id: string,
  contextId: string | null,
  signal?: AbortSignal,
) =>
  request(
    `${recordPath(id)}/status-transitions${
      contextId
        ? `?context_id=${encodeURIComponent(uuidSchema.parse(contextId))}`
        : ''
    }`,
    itemsResponseSchema(statusTransitionAccessSchema),
    signal === undefined ? undefined : { signal },
  );

export const getRecordApprovals = (id: string, signal?: AbortSignal) =>
  request(
    `${recordPath(id)}/approvals`,
    itemsResponseSchema(recordApprovalSchema),
    signal === undefined ? undefined : { signal },
  );

export const getRecordRetentionHolds = (id: string, signal?: AbortSignal) =>
  request(
    `${recordPath(id)}/retention-holds`,
    itemsResponseSchema(retentionHoldSchema),
    signal === undefined ? undefined : { signal },
  );
