import {
  readStoredJson,
  writeStoredJson,
  storageAreas,
} from '../../components/browserStorage';
import type { Attribute, BlueprintWithAttributes } from '../records/api';
import { isHiddenByDefault } from '../records/attributeVisibility';
import { attributeLabel } from '../records/recordDisplay';
import {
  browseByStorageKeyPrefix,
  explorerVisibilityScope,
  maximumBrowsePathHops,
  maximumBrowsePaths,
  relationshipPathLabelSeparator,
  relationshipPathSeparator,
} from './constants';
import {
  isRelationshipFilterAttribute,
  type RelationshipFilterAttribute,
} from './relationshipFilterTypes';
import { browseBySchema, type BrowseBy } from './search';
import { attributeValueTypes } from '../records/valueTypes';
import { attributeCardinalities } from '../records/constants';

/** A relationship field or path the browse rail can list targets of. */
export type BrowseByField = {
  /**
   * The field, or for a path a stand-in coded like `model.brand`, named by
   * its hops and targeting the last hop's target.
   */
  attribute: RelationshipFilterAttribute;
  /** Self-targeting field that arranges the targets as a tree. */
  hierarchyField?: string;
  /** The relationships a path follows, from the browsed blueprint on. */
  hops?: RelationshipFilterAttribute[];
  /**
   * `attribute` is the browsed blueprint's own hierarchy field, so the rail
   * lists the blueprint's records and a node matches itself and its subtree.
   */
  ownTree?: true;
};

/** Looks up a loaded blueprint by code. */
type BlueprintLookup = (code: string) => BlueprintWithAttributes | undefined;

const hierarchyPreference = { tree: 0, acyclic: 1 } as const;

/**
 * The target's self-targeting hierarchy field, preferring `tree` over
 * `acyclic`. The API accepts the same fields as a facet `hierarchy_field`.
 */
export const hierarchyFieldOf = (target: BlueprintWithAttributes) =>
  target.attributes
    .filter(
      (attribute) =>
        attribute.value_type === attributeValueTypes.relationship &&
        attribute.hierarchy != null &&
        attribute.target_blueprint_code === target.blueprint.code,
    )
    .sort(
      (left, right) =>
        hierarchyPreference[left.hierarchy!] -
        hierarchyPreference[right.hierarchy!],
    )[0]?.code;

const singleTarget = (attribute: RelationshipFilterAttribute) =>
  (attribute.target_blueprint_codes?.length ?? 1) <= 1;

/**
 * Whether a path can follow `attribute`: a single-valued relationship with a
 * single target blueprint that Explorer shows, as the API requires of every
 * hop of a path.
 */
const isPathHop = (
  attribute: Attribute,
): attribute is RelationshipFilterAttribute =>
  isRelationshipFilterAttribute(attribute) &&
  singleTarget(attribute) &&
  attribute.cardinality === attributeCardinalities.one &&
  !isHiddenByDefault(attribute, explorerVisibilityScope);

/**
 * Relationship fields with a single target blueprint. Fields that allow
 * several targets cannot supply one list of targets. The browsed blueprint's
 * own hierarchy field lists its own tree rather than its records' parents.
 */
export const browseByFields = (
  blueprintCode: string | undefined,
  relationshipFields: RelationshipFilterAttribute[],
  targetOf: BlueprintLookup,
): BrowseByField[] =>
  relationshipFields.flatMap((attribute) => {
    if (!singleTarget(attribute)) return [];
    const target = targetOf(attribute.target_blueprint_code);
    if (!target) return [];
    const hierarchyField = hierarchyFieldOf(target);
    const ownTree =
      target.blueprint.code === blueprintCode &&
      hierarchyField === attribute.code;
    return [
      { attribute, hierarchyField, ...(ownTree ? { ownTree: true } : {}) },
    ];
  });

/**
 * Each path extended by one hop through its last target's path hops, up to
 * `maximumBrowsePathHops`. Pass the direct fields as one-hop paths. A path
 * never returns to a blueprint it has passed, so a hierarchy or a link back
 * does not lead around in circles.
 */
export const extendBrowsePaths = (
  blueprintCode: string | undefined,
  paths: RelationshipFilterAttribute[][],
  targetOf: BlueprintLookup,
): RelationshipFilterAttribute[][] =>
  paths.flatMap((path) => {
    const last = path.at(-1);
    if (!last || path.length >= maximumBrowsePathHops || !isPathHop(last))
      return [];
    const passed = new Set([
      blueprintCode,
      ...path.map((hop) => hop.target_blueprint_code),
    ]);
    return (targetOf(last.target_blueprint_code)?.attributes ?? [])
      .filter(isPathHop)
      .filter((hop) => !passed.has(hop.target_blueprint_code))
      .map((hop) => [...path, hop]);
  });

/**
 * Rail fields for paths of two or more hops whose last target has loaded,
 * shortest first. Past `maximumBrowsePaths`, only the `kept` paths (such as
 * the current choice) stay, so a long list cannot crowd out a saved choice.
 */
export const browsePathFields = (
  paths: RelationshipFilterAttribute[][],
  targetOf: BlueprintLookup,
  kept: (BrowseBy | null | undefined)[] = [],
): BrowseByField[] =>
  paths
    .flatMap((hops): BrowseByField[] => {
      const last = hops.at(-1)!;
      const target = targetOf(last.target_blueprint_code);
      if (!target) return [];
      return [
        {
          attribute: {
            code: hops.map((hop) => hop.code).join(relationshipPathSeparator),
            name: hops
              .map((hop) => attributeLabel(hop))
              .join(relationshipPathLabelSeparator),
            cardinality: attributeCardinalities.one,
            target_blueprint_code: last.target_blueprint_code,
            value_type: attributeValueTypes.relationship,
          },
          hierarchyField: hierarchyFieldOf(target),
          hops,
        },
      ];
    })
    .sort((left, right) => left.hops!.length - right.hops!.length)
    .filter(
      (field, index) =>
        index < maximumBrowsePaths || kept.includes(field.attribute.code),
    );

/**
 * The field the rail lists: the URL's choice, then the person's last choice
 * for the blueprint, then the blueprint's `views.explorer.browse_by`, then its
 * own tree, then the first direct field whose target is a hierarchy. A choice
 * the current revision no longer offers falls through.
 */
export const resolveBrowseBy = (
  fields: BrowseByField[],
  requested: BrowseBy | undefined,
  stored: BrowseBy | undefined,
  blueprintDefault?: BrowseBy | null,
): BrowseByField | undefined => {
  for (const choice of [requested, stored]) {
    if (choice === false) return undefined;
    const field = fields.find((field) => field.attribute.code === choice);
    if (field) return field;
  }
  if (blueprintDefault === false) return undefined;
  return defaultBrowseByField(fields, blueprintDefault);
};

/**
 * The field the blueprint opens the rail on: its `views.explorer.browse_by`,
 * then its own tree, then the first direct field whose target is a
 * hierarchy. A path opens the rail only when the blueprint names it.
 */
export const defaultBrowseByField = (
  fields: BrowseByField[],
  blueprintDefault: BrowseBy | null | undefined,
): BrowseByField | undefined =>
  fields.find((field) => field.attribute.code === blueprintDefault) ??
  fields.find((field) => field.ownTree) ??
  fields.find((field) => field.hierarchyField && !field.hops);

const storageKey = (blueprintId: string) =>
  `${browseByStorageKeyPrefix}${blueprintId}`;

export const getStoredBrowseBy = (blueprintId: string): BrowseBy | undefined =>
  readStoredJson(storageAreas.local, storageKey(blueprintId), browseBySchema);

export const setStoredBrowseBy = (blueprintId: string, browseBy: BrowseBy) =>
  writeStoredJson(storageAreas.local, storageKey(blueprintId), browseBy);
