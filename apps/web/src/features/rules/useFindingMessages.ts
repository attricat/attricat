import { useQueries, useQuery } from '@tanstack/react-query';
import {
  blueprintCatalogueOptions,
  blueprintRevisionOptions,
} from '../blueprints/queryOptions';
import type { Finding } from './api';
import { findingMessage } from './findingMessage';
import type { RulesById } from './rulesById';

/**
 * Words each finding's message with the labels of the attributes and
 * options it names.
 * Findings span blueprints, so the attributes of the latest revision of each
 * blueprint their rules check are loaded; until they arrive, or when they
 * cannot be read, a message keeps the attribute's code.
 */
export const useFindingMessages = (
  findings: readonly Finding[],
  rules: RulesById,
) => {
  const catalogue = useQuery({
    ...blueprintCatalogueOptions(),
    enabled: findings.length > 0,
  });
  const blueprintIds = [
    ...new Set(
      findings.flatMap((finding) => {
        const rule = rules.get(finding.rule_id);
        return rule ? [rule.blueprint_id] : [];
      }),
    ),
  ];
  const blueprints = useQueries({
    queries: blueprintIds.map((id) =>
      blueprintRevisionOptions(
        id,
        catalogue.data?.find((blueprint) => blueprint.id === id)?.version,
      ),
    ),
  });
  const attributes = new Map(
    blueprintIds.map((id, index) => [
      id,
      blueprints[index]?.data?.attributes ?? [],
    ]),
  );
  return (finding: Finding) => {
    const rule = rules.get(finding.rule_id);
    return findingMessage(
      finding.message,
      (rule && attributes.get(rule.blueprint_id)) ?? [],
    );
  };
};

/** The attributes of a blueprint's latest revision, once loaded. */
export const useLatestBlueprintAttributes = (blueprintId: string) => {
  const catalogue = useQuery(blueprintCatalogueOptions());
  const version = catalogue.data?.find(
    (blueprint) => blueprint.id === blueprintId,
  )?.version;
  const revision = useQuery(blueprintRevisionOptions(blueprintId, version));
  return revision.data?.attributes ?? [];
};
