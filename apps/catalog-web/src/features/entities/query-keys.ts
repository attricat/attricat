export const entityQueryKeys = {
  blueprints: () => ['entity-blueprints'] as const,
  contexts: () => ['attribute-contexts'] as const,
  relationshipTargets: (blueprint: string | null | undefined) =>
    ['relationship-targets', blueprint] as const,
  form: (entityId: string) => ['entity-form', entityId] as const,
  preview: (entityId: string) => ['entity-preview', entityId] as const,
  search: (
    blueprint: string | undefined,
    version: number | undefined,
    query: string | undefined,
  ) => ['entities', blueprint, version, query] as const,
} as const;
