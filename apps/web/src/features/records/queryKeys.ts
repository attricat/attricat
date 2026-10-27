import type {
  RecordSearchFilter,
  RelationshipTreeFacetChildrenRequest,
} from './schemas';

type RecordSearchKeyOptions = {
  allVersions?: boolean;
  blueprint?: string;
  contextCode?: string;
  filters?: RecordSearchFilter[];
  query?: string;
  relationshipFilters?: { field: string; selected_target_ids: string[] }[];
  relationshipTreeFacets?: {
    field: string;
    hierarchyField: string;
    ownTree?: true;
    selectedIds: string[];
  }[];
  sort?: { field: string; direction: 'asc' | 'desc' };
  version?: number;
  exportStatus?: { extension: string; connection?: string; status: string };
};

export const recordQueryKeys = {
  searches: () => ['records'] as const,
  blueprints: () => ['record-blueprints'] as const,
  contexts: () => ['attribute-contexts'] as const,
  /**
   * A blueprint revision as records parse it. It is kept apart from the
   * blueprint pages' entry for the same revision, which a different schema
   * parses into a different shape.
   */
  blueprintRevision: (id: string | undefined, version: number | undefined) =>
    ['record-blueprint-revision', id, version] as const,
  blueprintsByCode: () => ['blueprint-by-code'] as const,
  blueprintByCode: (code: string | undefined, version: number | undefined) =>
    [...recordQueryKeys.blueprintsByCode(), code, version] as const,
  currentBlueprint: (id: string) => ['current-blueprint', id] as const,
  relationshipTreeFacetChildren: (
    input: Omit<RelationshipTreeFacetChildrenRequest, 'cursor'>,
  ) => ['relationship-tree-facet-children', input] as const,
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
  allResolvedPreviews: () => ['record-resolved-preview'] as const,
  resolvedPreviews: (recordId: string) =>
    [...recordQueryKeys.allResolvedPreviews(), recordId] as const,
  resolvedPreview: (recordId: string, contextId: string | undefined) =>
    [...recordQueryKeys.resolvedPreviews(recordId), contextId] as const,
  hierarchies: (recordId: string) => ['record-hierarchy', recordId] as const,
  hierarchy: (recordId: string, contextId: string, field: string) =>
    [...recordQueryKeys.hierarchies(recordId), contextId, field] as const,
  blueprintOverview: ({
    blueprint,
    contextCode,
    pageSize,
    query,
    version,
  }: {
    blueprint: string;
    contextCode: string;
    pageSize: number;
    query: string;
    version: number;
  }) =>
    [
      ...recordQueryKeys.searches(),
      'blueprint-overview',
      { blueprint, contextCode, pageSize, query, version },
    ] as const,
  search: ({
    allVersions = false,
    blueprint,
    contextCode,
    filters,
    query,
    relationshipFilters,
    relationshipTreeFacets,
    sort,
    version,
    exportStatus,
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
        relationshipTreeFacets,
        sort,
        version,
        exportStatus,
      },
    ] as const,
} as const;
