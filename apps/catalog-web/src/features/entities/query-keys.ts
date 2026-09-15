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
  publications: (entityId: string) =>
    ['entity-publications', entityId] as const,
  migrationPreview: (entityId: string) =>
    ['entity-migration-preview', entityId] as const,
  resolvedPreview: (entityId: string, contextId: string | undefined) =>
    ['entity-resolved-preview', entityId, contextId] as const,
  hierarchy: (entityId: string, contextId: string, field: string) =>
    ['entity-hierarchy', entityId, contextId, field] as const,
  incomingRelationships: (
    entityId: string,
    relationships: { source_blueprint: string; field: string }[],
    pageSize: number,
  ) => ['incoming-relationships', entityId, relationships, pageSize] as const,
  search: (
    blueprint: string | undefined,
    version: number | undefined,
    query: string | undefined,
    sort?: { field: string; direction: 'asc' | 'desc' },
    filters?: {
      field: string;
      operator: string;
      value: string | number | boolean;
    }[],
    allVersions = false,
    relationshipFilters?: { field: string; selected_target_ids: string[] }[],
  ) =>
    [
      ...entityQueryKeys.searches(),
      blueprint,
      version,
      query,
      sort,
      filters,
      allVersions,
      relationshipFilters,
    ] as const,
} as const;
