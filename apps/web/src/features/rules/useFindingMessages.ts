import { useQueries, useQuery } from '@tanstack/react-query';
import { revisionStatuses } from '../../api/revisionStatus';
import type { Blueprint } from '../blueprints/api';
import {
  blueprintRevisionListOptions,
  blueprintRevisionOptions,
} from '../blueprints/queryOptions';
import type { Finding } from './api';
import { findingMessage } from './findingMessage';
import type { RulesById } from './rulesById';

/**
 * The version rules check records against: the blueprint's latest published
 * revision, from its revisions listed newest first. A newer draft does not
 * count.
 */
export const latestPublishedVersion = (
  revisions: readonly Blueprint[] | undefined,
) =>
  revisions?.find((revision) => revision.status === revisionStatuses.published)
    ?.version;

/**
 * Words each finding's message with the labels of the attributes and
 * options it names.
 * Findings span blueprints, so the attributes of the latest published
 * revision of each blueprint their rules check are loaded; until they arrive, or when they
 * cannot be read, a message keeps the attribute's code.
 */
export const useFindingMessages = (
  findings: readonly Finding[],
  rules: RulesById,
) => {
  const blueprintIds = [
    ...new Set(
      findings.flatMap((finding) => {
        const rule = rules.get(finding.rule_id);
        return rule ? [rule.blueprint_id] : [];
      }),
    ),
  ];
  const revisionLists = useQueries({
    queries: blueprintIds.map((id) => blueprintRevisionListOptions(id)),
  });
  const blueprints = useQueries({
    queries: blueprintIds.map((id, index) =>
      blueprintRevisionOptions(
        id,
        latestPublishedVersion(revisionLists[index]?.data),
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

/** The attributes of a blueprint's latest published revision, once loaded. */
export const useLatestBlueprintAttributes = (blueprintId: string) => {
  const revisions = useQuery(blueprintRevisionListOptions(blueprintId));
  const revision = useQuery(
    blueprintRevisionOptions(
      blueprintId,
      latestPublishedVersion(revisions.data),
    ),
  );
  return revision.data?.attributes ?? [];
};
