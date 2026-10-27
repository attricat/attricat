import { z } from 'zod';
import { request, requestNoContent, jsonRequest } from '../../api/request';
import { activeLanguage } from '../../i18n';
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
  recordFormResponseSchema,
  recordMigrationPreviewSchema,
  recordPreviewResponseSchema,
  recordSchema,
  recordSearchResponseSchema,
  recordSearchPageSchema,
  relationshipTreeFacetChildrenRequestSchema,
  relationshipTreeFacetChildrenResponseSchema,
  systemTagFacetRequestSchema,
  systemTagFacetResponseSchema,
  blueprintSelectorSchema,
  getBlueprintRevisionRequestSchema,
  resolvedRecordPreviewSchema,
  smartFillRecordFormRequestSchema,
  smartFillRecordFormResponseSchema,
  searchRecordsRequestSchema,
  migrateRecordRequestSchema,
  updateRecordRequestSchema,
  recordPublicationStatusSchema,
  recordPublicationReadinessSchema,
  recordApprovalSchema,
  recordControlPageSchema,
  itemsResponseSchema,
  retentionHoldSchema,
  statusTransitionAccessSchema,
  uuidSchema,
  recordChangesPageSchema,
  recordsPublicationsResponseSchema,
} from './schemas';

import { uuidSegment } from '../../api/pathSegments';
import { keysetQuery, type KeysetCursor } from '../../api/keysetCursor';
export { viewBlockTypes } from '../views/schemas';
export type {
  ComponentReference,
  ViewDefinition,
  ViewNode,
} from '../views/schemas';
export type {
  Attribute,
  Blueprint,
  BlueprintWithAttributes,
  AttricatRecord,
  RecordApproval,
  RecordAuditChange,
  RecordFormResponse,
  RecordMigrationPreview,
  RecordPublicationReadiness,
  RecordPublicationStatus,
  FormAttributeValue,
  RecordItem,
  RecordSearchResponse,
  JsonSchema,
  NewAttributeValue,
  NewFileAttributeValue,
  RelationshipTargets,
  RelationshipTreeFacetChild,
  RelationshipTreeFacetChildrenRequest,
  SystemTagCount,
  SystemTagFacetRequest,
  SystemTagFacetResponse,
  ResolvedRecordPreview,
  RetentionHold,
  StatusTransitionAccess,
} from './schemas';

export { ApiRequestError } from '../../api/request';

/** Path of one record in the versioned record API. */
const recordPath = (id: string) => `/api/v1/records/${uuidSegment(id)}`;

type SearchRecordsOptions = {
  blueprint: string;
  /** Context whose values filters, sorting and table values resolve. */
  contextCode?: string;
  cursor?: string | null;
  filters?: import('./schemas').RecordSearchFilter[];
  includeTotal?: boolean;
  /** Defaults to `RECORD_SEARCH_PAGE_SIZE`. */
  pageSize?: number;
  query?: string;
  relationshipFilters?: {
    field: string;
    selected_target_ids: string[];
  }[];
  /** Relationship selections that also match descendants of each target. */
  relationshipTreeFacets?: {
    source_relationship_field?: string;
    hierarchy_field?: string;
    context_id: string;
    selected_target_ids: string[];
  }[];
  signal?: AbortSignal;
  sort?: { field: string; direction: 'asc' | 'desc'; context_code?: string };
  version?: number;
  /** Only records with this export status at one destination. */
  exportStatus?: {
    extension_id: string;
    connection?: string;
    status: string;
  };
  /** Only records that carry every one of these system tags. */
  systemTags?: string[];
};

const searchRecordsInit = (
  {
    blueprint,
    contextCode,
    cursor = null,
    filters = [],
    includeTotal = false,
    pageSize = RECORD_SEARCH_PAGE_SIZE,
    query = '',
    relationshipFilters = [],
    relationshipTreeFacets = [],
    signal,
    sort,
    version,
    exportStatus,
    systemTags = [],
  }: SearchRecordsOptions,
  include: { include_blueprint?: boolean; include_previews?: boolean } = {},
): RequestInit => {
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
    ...(relationshipTreeFacets.length
      ? { relationship_tree_facets: relationshipTreeFacets }
      : {}),
    ...(exportStatus ? { export_status: exportStatus } : {}),
    ...(systemTags.length ? { system_tags: systemTags } : {}),
    sort,
    ...(contextCode === undefined ? {} : { context_code: contextCode }),
    ...(includeTotal ? { include_total: true } : {}),
    ...include,
    page: { size: pageSize, cursor },
  });
  return {
    ...jsonRequest('POST', payload),
    ...(signal === undefined ? {} : { signal }),
  };
};

export const searchRecords = (options: SearchRecordsOptions) =>
  request(
    '/api/v1/records/search',
    recordSearchResponseSchema,
    searchRecordsInit(options),
  );

/**
 * A results page for a table: later pages leave out the blueprint the first
 * page carried, and rows carry previews and related records only when an
 * extension cell renderer needs them.
 */
export const searchRecordTablePage = (options: SearchRecordsOptions) =>
  request(
    '/api/v1/records/search',
    recordSearchPageSchema,
    searchRecordsInit(options, {
      include_blueprint: options.cursor == null,
      include_previews: false,
    }),
  );

/** One page of a relationship facet's root nodes or of one node's children. */
export const getRelationshipTreeFacetChildren = (
  input: import('./schemas').RelationshipTreeFacetChildrenRequest,
  signal?: AbortSignal,
) =>
  request(
    '/api/v1/records/facets/relationship-tree/children',
    relationshipTreeFacetChildrenResponseSchema,
    {
      ...jsonRequest(
        'POST',
        relationshipTreeFacetChildrenRequestSchema.parse(input),
      ),
      ...(signal === undefined ? {} : { signal }),
    },
  );

/** The system tags a blueprint's records carry, most used first. */
export const getSystemTagFacet = (
  input: import('./schemas').SystemTagFacetRequest,
  signal?: AbortSignal,
) =>
  request('/api/v1/records/facets/system-tags', systemTagFacetResponseSchema, {
    ...jsonRequest('POST', systemTagFacetRequestSchema.parse(input)),
    ...(signal === undefined ? {} : { signal }),
  });

export const listRecordBlueprints = (signal?: AbortSignal) =>
  request('/api/blueprints', z.array(blueprintSchema), { signal });
export const getRecordLabels = (
  recordIds: readonly string[],
  signal?: AbortSignal,
) =>
  request('/api/v1/records/labels', recordLabelsResponseSchema, {
    ...jsonRequest('POST', {
      record_ids: z.array(uuidSchema).parse(recordIds),
    }),
    ...(signal === undefined ? {} : { signal }),
  });
export const getRecordPreview = (id: string, signal?: AbortSignal) =>
  request(
    `/api/records/${uuidSegment(id)}/preview?relationship_depth=${RECORD_PREVIEW_QUERY.relationshipDepth}&relationship_limit=${RECORD_PREVIEW_QUERY.relationshipLimit}`,
    recordPreviewResponseSchema,
    { signal },
  );
export const getResolvedRecordPreview = (id: string, contextId: string) =>
  request(
    `/api/records/${uuidSegment(id)}/resolved-preview?context_id=${uuidSegment(contextId)}`,
    resolvedRecordPreviewSchema,
  );
export const getRecordHierarchy = (
  id: string,
  contextId: string,
  field: string,
) =>
  request(
    `/api/records/${uuidSegment(id)}/hierarchy?context_id=${uuidSegment(contextId)}&field=${encodeURIComponent(field)}`,
    recordHierarchySchema,
  );
export const getBlueprintByCode = (
  code: string,
  version?: number,
  signal?: AbortSignal,
) => {
  const input = blueprintSelectorSchema.parse({ code, version });
  const path = `/api/blueprints/by-code/${encodeURIComponent(input.code)}${input.version === undefined ? '' : `/versions/${input.version}`}`;
  return request(path, blueprintWithAttributesSchema, { signal });
};
export const getBlueprintRevision = (id: string, version: number) => {
  const input = getBlueprintRevisionRequestSchema.parse({ id, version });
  return request(
    `/api/blueprints/${encodeURIComponent(input.id)}/versions/${input.version}`,
    blueprintWithAttributesSchema,
  );
};
export const getCurrentBlueprint = (id: string) =>
  request(`/api/blueprints/${uuidSegment(id)}`, blueprintWithAttributesSchema);
export const getRecordChanges = (id: string, offset: number) =>
  request(
    `/api/records/${uuidSegment(id)}/changes?limit=${RECORD_CHANGES_PAGE_SIZE}&offset=${offset}`,
    recordChangesPageSchema,
  );
export const getRecordForm = (id: string, signal?: AbortSignal) =>
  request(recordPath(id), recordFormResponseSchema, { signal });
export const createRecord = (
  input: z.input<typeof createRecordRequestSchema>,
) => {
  const payload = createRecordRequestSchema.parse(input);
  return request('/api/v1/records', recordSchema, jsonRequest('POST', payload));
};
export const smartFillRecordForm = (
  input: z.input<typeof smartFillRecordFormRequestSchema>,
) => {
  const payload = smartFillRecordFormRequestSchema.parse({
    language: activeLanguage(),
    ...input,
  });
  return request(
    '/api/agent/smart-fill',
    smartFillRecordFormResponseSchema,
    jsonRequest('POST', payload),
  );
};

export const deleteRecord = (id: string) =>
  requestNoContent(`/api/records/${uuidSegment(id)}`, {
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
  return request(path, recordSchema, jsonRequest('PUT', payload));
};

export const getRecordPublications = (id: string, signal?: AbortSignal) =>
  request(
    `${recordPath(id)}/publications`,
    z.array(recordPublicationStatusSchema),
    { signal },
  );
/** Statuses of up to 100 records; unreadable and unknown records are left out. */
export const getRecordsPublications = (
  recordIds: string[],
  signal?: AbortSignal,
) =>
  request('/api/v1/records/publications', recordsPublicationsResponseSchema, {
    ...jsonRequest('POST', { record_ids: recordIds }),
    ...(signal === undefined ? {} : { signal }),
  });
/** Whether the record passes each enabled channel's publication checks. */
export const getRecordPublicationReadiness = (
  id: string,
  signal?: AbortSignal,
) =>
  request(
    `${recordPath(id)}/publications/readiness`,
    z.array(recordPublicationReadinessSchema),
    { signal },
  );
export const publishRecord = (id: string, contextId: string) =>
  request(
    `${recordPath(id)}/publications`,
    recordPublicationStatusSchema,
    jsonRequest('POST', { context_id: uuidSchema.parse(contextId) }),
  );
/** Moves a locking status to `to` outside the declared transitions. */
export const unlockRecordStatus = (
  id: string,
  input: {
    attributeCode: string;
    contextId: string | null;
    to: string;
    expectedUpdatedAt: string;
  },
) =>
  request(
    `${recordPath(id)}/status-unlock`,
    recordSchema,
    jsonRequest('POST', {
      attribute_code: input.attributeCode,
      ...(input.contextId && { context_id: uuidSchema.parse(input.contextId) }),
      to: input.to,
      expected_updated_at: input.expectedUpdatedAt,
    }),
  );
export const publishRecordAllChannels = (id: string) =>
  request(
    `${recordPath(id)}/publications/publish-all`,
    z.array(recordPublicationStatusSchema),
    { method: 'POST' },
  );
export const unpublishRecord = (id: string, contextId: string) =>
  requestNoContent(
    `${recordPath(id)}/publications/unpublish`,
    jsonRequest('POST', { context_id: uuidSchema.parse(contextId) }),
  );
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
  return request(
    `${path}/blueprint-migration${query}`,
    recordSchema,
    jsonRequest('POST', payload),
  );
};

/** Declared edges from the saved status and whether the caller may take them. */
export const getStatusTransitions = (
  id: string,
  contextId: string | null,
  signal?: AbortSignal,
) =>
  request(
    `${recordPath(id)}/status-transitions${
      contextId ? `?context_id=${uuidSegment(contextId)}` : ''
    }`,
    itemsResponseSchema(statusTransitionAccessSchema),
    { signal },
  );

export const getRecordApprovals = (
  id: string,
  cursor?: KeysetCursor,
  signal?: AbortSignal,
) =>
  request(
    `${recordPath(id)}/approvals${keysetQuery(cursor)}`,
    recordControlPageSchema(recordApprovalSchema),
    { signal },
  );

export const getRecordRetentionHolds = (
  id: string,
  cursor?: KeysetCursor,
  signal?: AbortSignal,
) =>
  request(
    `${recordPath(id)}/retention-holds${keysetQuery(cursor)}`,
    recordControlPageSchema(retentionHoldSchema),
    { signal },
  );
