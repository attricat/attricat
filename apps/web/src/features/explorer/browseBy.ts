import type { BlueprintWithAttributes } from '../records/api';
import { browseByStorageKeyPrefix } from './constants';
import type { RelationshipFilterAttribute } from './relationshipFilterTypes';
import { browseBySchema, type BrowseBy } from './search';

/** A relationship field the browse rail can list targets of. */
export type BrowseByField = {
  attribute: RelationshipFilterAttribute;
  /** Self-targeting field that arranges the targets as a tree. */
  hierarchyField?: string;
};

const hierarchyPreference = { tree: 0, acyclic: 1 } as const;

/**
 * The target's self-targeting hierarchy field, preferring `tree` over
 * `acyclic`. The API accepts the same fields as a facet `hierarchy_field`.
 */
export const hierarchyFieldOf = (target: BlueprintWithAttributes) =>
  target.attributes
    .filter(
      (attribute) =>
        attribute.value_type === 'relationship' &&
        attribute.hierarchy != null &&
        attribute.target_blueprint_code === target.blueprint.code,
    )
    .sort(
      (left, right) =>
        hierarchyPreference[left.hierarchy!] -
        hierarchyPreference[right.hierarchy!],
    )[0]?.code;

/**
 * Relationship fields with a single target blueprint. Fields that allow
 * several targets cannot supply one list of targets.
 */
export const browseByFields = (
  relationshipFields: RelationshipFilterAttribute[],
  targets: (BlueprintWithAttributes | undefined)[],
): BrowseByField[] =>
  relationshipFields.flatMap((attribute, index) => {
    if ((attribute.target_blueprint_codes?.length ?? 1) > 1) return [];
    const target = targets[index];
    if (!target) return [];
    return [{ attribute, hierarchyField: hierarchyFieldOf(target) }];
  });

/**
 * The field the rail lists: the URL's choice, then the person's last choice
 * for the blueprint, then the blueprint's `views.explorer.browse_by`, then the
 * first field whose target is a hierarchy. A choice the current revision no
 * longer offers falls through.
 */
export const resolveBrowseBy = (
  fields: BrowseByField[],
  requested: BrowseBy | undefined,
  stored: BrowseBy | undefined,
  blueprintDefault?: string | null,
): BrowseByField | undefined => {
  for (const choice of [requested, stored, blueprintDefault]) {
    if (choice === false) return undefined;
    const field = fields.find((field) => field.attribute.code === choice);
    if (field) return field;
  }
  return fields.find((field) => field.hierarchyField);
};

const storageKey = (blueprintId: string) =>
  `${browseByStorageKeyPrefix}${blueprintId}`;

export const getStoredBrowseBy = (
  blueprintId: string,
): BrowseBy | undefined => {
  try {
    const stored = localStorage.getItem(storageKey(blueprintId));
    if (stored === null) return undefined;
    const parsed = browseBySchema.safeParse(JSON.parse(stored));
    return parsed.success ? parsed.data : undefined;
  } catch {
    return undefined;
  }
};

export const setStoredBrowseBy = (blueprintId: string, browseBy: BrowseBy) => {
  try {
    localStorage.setItem(storageKey(blueprintId), JSON.stringify(browseBy));
  } catch {
    // Storage can be unavailable in hardened browser contexts.
  }
};
