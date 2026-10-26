import { useInfiniteQuery, useQuery } from '@tanstack/react-query';
import { useMemo } from 'react';
import { capabilities } from '../auth/capabilities';
import { useCan } from '../auth/useCan';
import {
  recordFindingsOptions,
  ruleDefinitionsOptions,
} from '../rules/queryOptions';
import type { Attribute } from './api';
import { blueprintConnections } from './attributeConnections';
import type { FieldDiagnostics } from './fieldDiagnostics';

/**
 * Field diagnostics for a blueprint's fields: what ties them together, plus
 * the data quality rules enabled for a saved `revision` and, for a saved
 * `record`, its findings in the context shown.
 */
export const useBlueprintFieldDiagnostics = ({
  attributes,
  recordSchema,
  definition,
  revision,
  record,
}: {
  attributes: readonly Attribute[];
  recordSchema: unknown;
  /** The blueprint's definition source, for what only it records. */
  definition: unknown;
  revision?: { id: string; version: number };
  record?: { id: string; contextCode?: string; contextId?: string };
}): FieldDiagnostics => {
  const canReadRules = useCan(capabilities.rulesRead);
  const findings = useInfiniteQuery({
    ...recordFindingsOptions(record?.id ?? ''),
    enabled: canReadRules && record !== undefined,
  });
  const rules = useQuery({
    ...ruleDefinitionsOptions(),
    enabled: canReadRules && revision !== undefined,
  });
  const revisionId = revision?.id;
  const revisionVersion = revision?.version;
  const connections = useMemo(
    () =>
      blueprintConnections({
        attributes,
        recordSchema,
        definition,
        rules:
          revisionId === undefined
            ? undefined
            : rules.data?.filter(
                (rule) =>
                  rule.blueprint_id === revisionId &&
                  rule.blueprint_version === revisionVersion &&
                  rule.enabled_version !== null,
              ),
      }),
    [
      attributes,
      definition,
      recordSchema,
      revisionId,
      revisionVersion,
      rules.data,
    ],
  );
  const contextCode = record?.contextCode;
  const contextId = record?.contextId;
  const showsRecord = record !== undefined;
  return useMemo(
    () => ({
      attributes,
      connections,
      contextCode,
      contextId,
      findings:
        canReadRules && showsRecord
          ? (findings.data?.pages.flatMap((page) => page.items) ?? [])
          : undefined,
      showsValues: showsRecord,
    }),
    [
      attributes,
      canReadRules,
      connections,
      contextCode,
      contextId,
      findings.data,
      showsRecord,
    ],
  );
};
