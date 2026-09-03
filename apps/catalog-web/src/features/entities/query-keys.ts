export const entityQueryKeys = {
  blueprints: () => ['entity-blueprints'] as const,
  contexts: () => ['attribute-contexts'] as const,
  blueprintRevision: (id: string, version: number) =>
    ['blueprint-revision', id, version] as const,
  blueprintByCode: (code: string | undefined, version: number | undefined) =>
    ['blueprint-by-code', code, version] as const,
  currentBlueprint: (id: string) => ['current-blueprint', id] as const,
  relationshipTargets: (blueprint: string | null | undefined) =>
    ['relationship-targets', blueprint] as const,
  form: (entityId: string) => ['entity-form', entityId] as const,
  changes: (entityId: string) => ['entity-changes', entityId] as const,
  migrationPreview: (entityId: string) =>
    ['entity-migration-preview', entityId] as const,
  resolvedPreview: (entityId: string, contextId: string) =>
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
    relationshipTreeFacets?: {
      source_relationship_field: string;
      hierarchy_field?: string;
      context_id: string;
      selected_target_ids: string[];
    }[],
  ) => ['entities', blueprint, version, query, relationshipTreeFacets] as const,
  relationshipTreeFacetChildren: (
    blueprint: string,
    version: number | undefined,
    query: string | undefined,
    sourceField: string,
    hierarchyField: string,
    contextId: string,
    parentId: string | undefined,
    cursor: string | null,
  ) =>
    [
      'relationship-tree-facet-children',
      blueprint,
      version,
      query,
      sourceField,
      hierarchyField,
      contextId,
      parentId,
      cursor,
    ] as const,
} as const;
