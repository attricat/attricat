import { useQueries, type UseQueryResult } from '@tanstack/react-query';
import {
  getBlueprintByCode,
  type BlueprintWithAttributes,
} from '../records/api';
import { isHiddenByDefault } from '../records/attributeVisibility';
import { recordQueryKeys } from '../records/queryKeys';
import {
  explorerVisibilityScope,
  relationshipPathSeparator,
} from './constants';
import {
  isRelationshipFilterAttribute,
  type RelationshipFilterAttribute,
} from './relationshipFilterTypes';

const targetBlueprintQueries = (
  relationships: RelationshipFilterAttribute[],
  enabled: boolean,
) =>
  relationships.map((attribute) => ({
    queryKey: recordQueryKeys.blueprintByCode(
      attribute.target_blueprint_code,
      undefined,
    ),
    queryFn: ({ signal }: { signal: AbortSignal }) =>
      getBlueprintByCode(attribute.target_blueprint_code, undefined, signal),
    enabled,
  }));

const nestedRelationships = (
  targets: UseQueryResult<BlueprintWithAttributes>[],
  parents: RelationshipFilterAttribute[],
): RelationshipFilterAttribute[] =>
  targets.flatMap((result, index) =>
    (result.data?.attributes ?? [])
      .filter(isRelationshipFilterAttribute)
      .filter(
        (attribute) => !isHiddenByDefault(attribute, explorerVisibilityScope),
      )
      .map((attribute) => ({
        ...attribute,
        code: `${parents[index].code}${relationshipPathSeparator}${attribute.code}`,
      })),
  );

/**
 * Discovers relationship paths up to three hops away from the direct
 * relationships of the current blueprint.
 */
export const useRelationshipFilterPaths = (
  directRelationships: RelationshipFilterAttribute[],
  enabled: boolean,
) => {
  const firstTargets = useQueries({
    queries: targetBlueprintQueries(directRelationships, enabled),
  });
  const secondRelationships = nestedRelationships(
    firstTargets,
    directRelationships,
  );
  const secondTargets = useQueries({
    queries: targetBlueprintQueries(secondRelationships, enabled),
  });
  const thirdRelationships = nestedRelationships(
    secondTargets,
    secondRelationships,
  );
  const loading =
    enabled &&
    [...firstTargets, ...secondTargets].some((result) => result.isFetching);

  return {
    loading,
    relationshipPaths: [...secondRelationships, ...thirdRelationships],
  };
};
