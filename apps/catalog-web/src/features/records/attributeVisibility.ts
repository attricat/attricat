export const attributeVisibilityTags = {
  hidden: 'hidden',
  form: 'hidden:form',
  detail: 'hidden:detail',
  explorer: 'hidden:explorer',
  metadata: 'hidden:metadata',
} as const;

export type AttributeVisibilityScope = Exclude<
  keyof typeof attributeVisibilityTags,
  'hidden'
>;

type TaggableAttribute = {
  tags?: readonly string[];
};

export const isHiddenByDefault = (
  attribute: TaggableAttribute,
  scope: AttributeVisibilityScope,
) => {
  const tags = attribute.tags ?? [];
  return (
    tags.includes(attributeVisibilityTags.hidden) ||
    tags.includes(attributeVisibilityTags[scope])
  );
};
