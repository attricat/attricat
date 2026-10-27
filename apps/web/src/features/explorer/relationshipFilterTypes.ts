import type { Attribute } from '../records/api';
import { attributeValueTypes } from '../records/valueTypes';

export type RelationshipFilterAttribute = Attribute & {
  target_blueprint_code: string;
  value_type: typeof attributeValueTypes.relationship;
};

export const isRelationshipFilterAttribute = (
  attribute: Attribute,
): attribute is RelationshipFilterAttribute =>
  attribute.value_type === attributeValueTypes.relationship &&
  typeof attribute.target_blueprint_code === 'string';

export type ExplorerRelationshipFacet = {
  /** Selects the browsed blueprint's own records and their descendants. */
  ownTree?: true;
  selectedIds: string[];
  sourceRelationship: RelationshipFilterAttribute;
};

export type RelationshipFacetUpdate = {
  hierarchyField?: string;
  ownTree?: true;
  selectedIds?: string[];
  targetBlueprint?: string;
};
