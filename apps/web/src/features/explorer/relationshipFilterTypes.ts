import type { Attribute } from '../records/api';
import { allowedTargetBlueprints } from '../records/relationshipTargets';
import { attributeValueTypes } from '../records/valueTypes';

/**
 * A relationship the Explorer can filter by: one that names its target
 * blueprints. A relationship to several blueprints filters by records of any
 * of them; `target_blueprint_code` is set only when there is one.
 */
export type RelationshipFilterAttribute = Attribute & {
  value_type: typeof attributeValueTypes.relationship;
};

export const isRelationshipFilterAttribute = (
  attribute: Attribute,
): attribute is RelationshipFilterAttribute =>
  attribute.value_type === attributeValueTypes.relationship &&
  allowedTargetBlueprints(attribute).length > 0;

/** A relationship filter that leads to exactly one blueprint. */
export type SingleTargetRelationshipFilterAttribute =
  RelationshipFilterAttribute & { target_blueprint_code: string };

export const hasSingleFilterTarget = (
  attribute: RelationshipFilterAttribute,
): attribute is SingleTargetRelationshipFilterAttribute =>
  typeof attribute.target_blueprint_code === 'string' &&
  (attribute.target_blueprint_codes?.length ?? 1) <= 1;
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
