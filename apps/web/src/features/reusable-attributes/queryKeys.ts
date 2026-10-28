export const reusableAttributeQueryKeys = {
  root: () => ['reusable-attributes'] as const,
  definitions: (includeDrafts = false) =>
    [...reusableAttributeQueryKeys.root(), { includeDrafts }] as const,
  filterFields: (blueprint: string) =>
    [...reusableAttributeQueryKeys.root(), 'filter-fields', blueprint] as const,
  groups: () => ['reusable-attribute-groups'] as const,
} as const;
