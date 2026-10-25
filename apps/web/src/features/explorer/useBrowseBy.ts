import { useQueries } from '@tanstack/react-query';
import { type BlueprintWithAttributes } from '../records/api';
import {
  browseByFields,
  browsePathFields,
  defaultBrowseByField,
  extendBrowsePaths,
  getStoredBrowseBy,
  resolveBrowseBy,
  setStoredBrowseBy,
} from './browseBy';
import type { RelationshipFilterAttribute } from './relationshipFilterTypes';
import type { BrowseBy } from './search';
import { blueprintByCodeOptions } from '../records/queryOptions';

/** The latest revisions of the given blueprints. */
const useBlueprints = (codes: string[]) =>
  // Shares cache entries with relationship filter path discovery.
  useQueries({
    queries: [...new Set(codes)].map((code) => blueprintByCodeOptions(code)),
  });

const lastTargets = (paths: RelationshipFilterAttribute[][]) =>
  paths.map((path) => path.at(-1)!.target_blueprint_code);

/**
 * The browse rail's candidate fields and current choice for one blueprint:
 * its direct relationship fields, then relationship paths through
 * single-valued hops. A change is written to the URL, so saved views keep
 * it, and remembered as the person's default for the blueprint.
 */
export const useBrowseBy = ({
  blueprintCode,
  blueprintDefault,
  blueprintId,
  onChange,
  relationshipFields,
  requested,
}: {
  blueprintCode: string | undefined;
  /** The blueprint's `views.explorer.browse_by`. */
  blueprintDefault: BrowseBy | null | undefined;
  blueprintId: string | undefined;
  onChange: (browseBy: BrowseBy) => void;
  relationshipFields: RelationshipFilterAttribute[];
  requested: BrowseBy | undefined;
}) => {
  // Each level loads the targets the next hop starts from.
  const blueprints = new Map<string, BlueprintWithAttributes>();
  const targetOf = (code: string) => blueprints.get(code);
  const collect = <Result extends { data?: BlueprintWithAttributes }>(
    results: Result[],
  ) => {
    for (const result of results) {
      if (result.data) blueprints.set(result.data.blueprint.code, result.data);
    }
    return results;
  };
  const direct = collect(
    useBlueprints(
      relationshipFields.map((attribute) => attribute.target_blueprint_code),
    ),
  );
  const twoHops = extendBrowsePaths(
    blueprintCode,
    relationshipFields.map((attribute) => [attribute]),
    targetOf,
  );
  const second = collect(useBlueprints(lastTargets(twoHops)));
  const threeHops = extendBrowsePaths(blueprintCode, twoHops, targetOf);
  const third = collect(useBlueprints(lastTargets(threeHops)));

  const stored =
    blueprintId === undefined ? undefined : getStoredBrowseBy(blueprintId);
  const fields = [
    ...browseByFields(blueprintCode, relationshipFields, targetOf),
    ...browsePathFields([...twoHops, ...threeHops], targetOf, [
      requested,
      stored,
      blueprintDefault,
    ]),
  ];
  const pending = [...direct, ...second, ...third].some(
    (target) => target.isPending,
  );

  return {
    field: pending
      ? undefined
      : resolveBrowseBy(fields, requested, stored, blueprintDefault),
    /** The field the rail reopens on after the person hides it. */
    defaultField: defaultBrowseByField(fields, blueprintDefault) ?? fields[0],
    fields,
    pending,
    change: (browseBy: BrowseBy) => {
      if (blueprintId !== undefined) setStoredBrowseBy(blueprintId, browseBy);
      onChange(browseBy);
    },
  };
};
