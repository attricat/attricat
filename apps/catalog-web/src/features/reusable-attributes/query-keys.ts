export const reusableAttributeQueryKeys = {
  root: () => ['reusable-attributes'] as const,
  definitions: (includeDrafts = false) =>
    [...reusableAttributeQueryKeys.root(), { includeDrafts }] as const,
  groups: () => ['reusable-attribute-groups'] as const,
} as const;
