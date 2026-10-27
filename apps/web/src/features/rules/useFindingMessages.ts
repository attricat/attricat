import { useQueries } from '@tanstack/react-query';
import { blueprintRevisionOptions } from '../blueprints/queryOptions';
import type { Finding } from './api';
import { findingMessage } from './findingMessage';
import { ruleRevisionKey, type RuleRevisions } from './ruleRevisions';
import { findingEvidenceSchema } from './schemas';

const scopeKey = (blueprintId: string, version: number) =>
  `${blueprintId}:${version}`;

/**
 * Words each finding's message with the label of the attribute it is about.
 * Findings span blueprints, so the attributes of each blueprint revision their
 * rules check are loaded; until they arrive, or when they cannot be read, a
 * message keeps the attribute's code.
 */
export const useFindingMessages = (
  findings: readonly Finding[],
  revisions: RuleRevisions,
) => {
  const ruleOf = (finding: Finding) =>
    revisions.get(ruleRevisionKey(finding.rule_id, finding.rule_version));
  const scopes = new Map<string, { id: string; version: number }>();
  for (const finding of findings) {
    const rule = ruleOf(finding);
    if (rule)
      scopes.set(scopeKey(rule.blueprint_id, rule.blueprint_version), {
        id: rule.blueprint_id,
        version: rule.blueprint_version,
      });
  }
  const blueprints = useQueries({
    queries: [...scopes.values()].map(({ id, version }) =>
      blueprintRevisionOptions(id, version),
    ),
  });
  const attributes = new Map(
    [...scopes.keys()].map((key, index) => [
      key,
      blueprints[index]?.data?.attributes ?? [],
    ]),
  );
  return (finding: Finding) => {
    const rule = ruleOf(finding);
    const evidence = findingEvidenceSchema.safeParse(finding.evidence);
    return findingMessage(
      finding.message,
      evidence.success ? evidence.data.attribute_code : undefined,
      (rule &&
        attributes.get(scopeKey(rule.blueprint_id, rule.blueprint_version))) ??
        [],
    );
  };
};
