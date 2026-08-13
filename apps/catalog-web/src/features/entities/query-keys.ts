export const entityQueryKeys = {
  blueprints: () => ['entity-blueprints'] as const,
  contexts: () => ['attribute-contexts'] as const,
  blueprintRevision: (id: string, version: number) =>
    ['blueprint-revision', id, version] as const,
  relationshipTargets: (blueprint: string | null | undefined) =>
    ['relationship-targets', blueprint] as const,
  form: (entityId: string) => ['entity-form', entityId] as const,
  preview: (entityId: string) => ['entity-preview', entityId] as const,
  resolvedPreview: (entityId: string, contextId: string) =>
    ['entity-resolved-preview', entityId, contextId] as const,
  search: (
    blueprint: string | undefined,
    version: number | undefined,
    query: string | undefined,
  ) => ['entities', blueprint, version, query] as const,
} as const;
