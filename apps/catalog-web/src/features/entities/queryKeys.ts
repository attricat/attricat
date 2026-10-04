import type { EntitySearchFilter } from './schemas';

type EntitySearchKeyOptions = {
  allVersions?: boolean;
  blueprint?: string;
  filters?: EntitySearchFilter[];
  query?: string;
  relationshipFilters?: { field: string; selected_target_ids: string[] }[];
  sort?: { field: string; direction: 'asc' | 'desc' };
  version?: number;
};

export const entityQueryKeys = {
  searches: () => ['entities'] as const,
  blueprints: () => ['entity-blueprints'] as const,
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
  form: (entityId: string) => ['entity-form', entityId] as const,
  preview: (entityId: string) => ['entity-preview', entityId] as const,
  changes: (entityId: string) => ['entity-changes', entityId] as const,
  recordControls: (entityId: string) =>
    ['entity-record-controls', entityId] as const,
  statusTransitions: (entityId: string, contextId: string | null) =>
    [
      ...entityQueryKeys.recordControls(entityId),
      'transitions',
      contextId,
    ] as const,
  publications: () => ['entity-publications'] as const,
  publication: (entityId: string) =>
    [...entityQueryKeys.publications(), entityId] as const,
  publicationReadiness: (entityId: string) =>
    [...entityQueryKeys.publication(entityId), 'readiness'] as const,
  migrationPreview: (entityId: string) =>
    ['entity-migration-preview', entityId] as const,
  resolvedPreviews: (entityId: string) =>
    ['entity-resolved-preview', entityId] as const,
  resolvedPreview: (entityId: string, contextId: string | undefined) =>
    [...entityQueryKeys.resolvedPreviews(entityId), contextId] as const,
  hierarchies: (entityId: string) => ['entity-hierarchy', entityId] as const,
  hierarchy: (entityId: string, contextId: string, field: string) =>
    [...entityQueryKeys.hierarchies(entityId), contextId, field] as const,
  incomingRelationshipResults: (entityId: string) =>
    ['incoming-relationships', entityId] as const,
  incomingRelationships: (
    entityId: string,
    relationships: { source_blueprint: string; field: string }[],
    pageSize: number,
  ) =>
    [
      ...entityQueryKeys.incomingRelationshipResults(entityId),
      relationships,
      pageSize,
    ] as const,
  search: ({
    allVersions = false,
    blueprint,
    filters,
    query,
    relationshipFilters,
    sort,
    version,
  }: EntitySearchKeyOptions) =>
    [
      ...entityQueryKeys.searches(),
      {
        allVersions,
        blueprint,
        filters,
        query,
        relationshipFilters,
        sort,
        version,
      },
    ] as const,
} as const;
