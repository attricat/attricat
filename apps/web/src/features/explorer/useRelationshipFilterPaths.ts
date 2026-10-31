import { useQueries, type UseQueryResult } from '@tanstack/react-query';
import { type BlueprintWithAttributes } from '../records/api';
import { isHiddenByDefault } from '../records/attributeVisibility';
import {
  explorerVisibilityScope,
  relationshipPathSeparator,
} from './constants';
import {
  isRelationshipFilterAttribute,
  type RelationshipFilterAttribute,
} from './relationshipFilterTypes';
import { blueprintByCodeOptions } from '../records/queryOptions';

/**
 * Each relationship's target blueprint, in the relationships' order. Several
 * relationships can share a target, which is read once.
 */
const useTargetBlueprints = (
  relationships: RelationshipFilterAttribute[],
  enabled: boolean,
) => {
  const codes = [
    ...new Set(
      relationships.map((attribute) => attribute.target_blueprint_code),
    ),
  ];
  const results = useQueries({
    queries: codes.map((code) => ({
      ...blueprintByCodeOptions(code),
      enabled,
    })),
  });
  return relationships.map(
    (attribute) => results[codes.indexOf(attribute.target_blueprint_code)],
  );
};

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
  const firstTargets = useTargetBlueprints(directRelationships, enabled);
  const secondRelationships = nestedRelationships(
    firstTargets,
    directRelationships,
  );
  const secondTargets = useTargetBlueprints(secondRelationships, enabled);
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
