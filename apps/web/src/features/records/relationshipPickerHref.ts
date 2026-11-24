import { defaultStringifySearch } from '@tanstack/react-router';
import type { relationshipPickerModes } from './constants';
import type { RelationshipTargetFilter } from './components/relationshipScope';

/**
 * The Explorer tab a relationship picker opens to choose records of
 * `blueprint` with its table selection; the tab sends them back to the
 * picker `token` names. See RelationshipPickerSelectionBar.
 */
export const relationshipPickerExplorerHref = ({
  blueprint,
  mode,
  targetFilters,
  token,
}: {
  blueprint: string;
  mode: (typeof relationshipPickerModes)[keyof typeof relationshipPickerModes];
  /** Narrows the offered targets, such as a relationship's `target_scope`. */
  targetFilters?: readonly RelationshipTargetFilter[];
  token: string;
}) =>
  `/${defaultStringifySearch({
    blueprint,
    locked: true,
    ...(targetFilters?.length
      ? {
          relationshipFacets: targetFilters.map((filter) => ({
            field: filter.field,
            selectedIds: filter.selected_target_ids,
          })),
        }
      : {}),
    relationshipPicker: token,
    relationshipPickerMode: mode,
  })}`;
