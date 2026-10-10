import type { RecordSearchFilter } from './schemas';

type RecordSearchKeyOptions = {
  allVersions?: boolean;
  blueprint?: string;
  contextCode?: string;
  filters?: RecordSearchFilter[];
  query?: string;
  relationshipFilters?: { field: string; selected_target_ids: string[] }[];
  sort?: { field: string; direction: 'asc' | 'desc' };
  version?: number;
};

export const recordQueryKeys = {
  searches: () => ['records'] as const,
  blueprints: () => ['record-blueprints'] as const,
  contexts: () => ['attribute-contexts'] as const,
  blueprintRevision: (id: string | undefined, version: number | undefined) =>
    ['blueprint-revision', id, version] as const,
  blueprintByCode: (code: string | undefined, version: number | undefined) =>
    ['blueprint-by-code', code, version] as const,
  currentBlueprint: (id: string) => ['current-blueprint', id] as const,
  blueprintRevisions: (id: string | undefined) =>
    ['blueprint-revisions', id] as const,
  relationshipTargets: (blueprint: string | null | undefined, query: string) =>
    ['relationship-targets', blueprint, query] as const,
  form: (recordId: string) => ['record-form', recordId] as const,
  preview: (recordId: string) => ['record-preview', recordId] as const,
  allLabels: () => ['record-labels'] as const,
  labels: (recordIds: readonly string[]) =>
    [...recordQueryKeys.allLabels(), ...recordIds] as const,
  changes: (recordId: string) => ['record-changes', recordId] as const,
  recordControls: (recordId: string) => ['record-controls', recordId] as const,
  approvals: (recordId: string) =>
    [...recordQueryKeys.recordControls(recordId), 'approvals'] as const,
  retentionHolds: (recordId: string) =>
    [...recordQueryKeys.recordControls(recordId), 'retention-holds'] as const,
  statusTransitions: (recordId: string, contextId: string | null) =>
    [
      ...recordQueryKeys.recordControls(recordId),
      'transitions',
      contextId,
    ] as const,
  publications: () => ['record-publications'] as const,
  publication: (recordId: string) =>
    [...recordQueryKeys.publications(), recordId] as const,
  publicationReadiness: (recordId: string) =>
    [...recordQueryKeys.publication(recordId), 'readiness'] as const,
  migrationPreview: (recordId: string) =>
    ['record-migration-preview', recordId] as const,
  resolvedPreviews: (recordId: string) =>
    ['record-resolved-preview', recordId] as const,
  resolvedPreview: (recordId: string, contextId: string | undefined) =>
    [...recordQueryKeys.resolvedPreviews(recordId), contextId] as const,
  hierarchies: (recordId: string) => ['record-hierarchy', recordId] as const,
  hierarchy: (recordId: string, contextId: string, field: string) =>
    [...recordQueryKeys.hierarchies(recordId), contextId, field] as const,
  incomingRelationshipResults: (recordId: string) =>
    ['incoming-relationships', recordId] as const,
  incomingRelationships: (
    recordId: string,
    relationships: { source_blueprint: string; field: string }[],
    pageSize: number,
  ) =>
    [
      ...recordQueryKeys.incomingRelationshipResults(recordId),
      relationships,
      pageSize,
    ] as const,
  search: ({
    allVersions = false,
    blueprint,
    contextCode,
    filters,
    query,
    relationshipFilters,
    sort,
    version,
  }: RecordSearchKeyOptions) =>
    [
      ...recordQueryKeys.searches(),
      {
        allVersions,
        blueprint,
        contextCode,
        filters,
        query,
        relationshipFilters,
        sort,
        version,
      },
    ] as const,
} as const;
