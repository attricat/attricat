import type { Attribute } from '../entities/api';

export type RelationshipFilterAttribute = Attribute & {
  target_blueprint_code: string;
  value_type: 'relationship';
};

export const isRelationshipFilterAttribute = (
  attribute: Attribute,
): attribute is RelationshipFilterAttribute =>
  attribute.value_type === 'relationship' &&
  typeof attribute.target_blueprint_code === 'string';

export type ExplorerRelationshipFacet = {
  selectedIds: string[];
  sourceRelationship: RelationshipFilterAttribute;
};

export type RelationshipFacetUpdate = {
  selectedIds?: string[];
  targetBlueprint?: string;
};
