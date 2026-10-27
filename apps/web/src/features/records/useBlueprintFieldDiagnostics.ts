import { useQuery } from '@tanstack/react-query';
import { useMemo } from 'react';
import { capabilities } from '../auth/capabilities';
import { useCan } from '../auth/useCan';
import { ruleDefinitionsOptions } from '../rules/queryOptions';
import type { Attribute } from './api';
import { blueprintConnections } from './attributeConnections';
import {
  fieldFindingsStatuses,
  type FieldDiagnostics,
} from './fieldDiagnostics';
import { recordDiagnosticFindingsOptions } from './queryOptions';

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
  const findings = useQuery({
    ...recordDiagnosticFindingsOptions(record?.id ?? ''),
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
                  rule.enabled &&
                  !rule.not_applicable.some(
                    (skipped) => skipped.blueprint_version === revisionVersion,
                  ),
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
  const readsFindings = canReadRules && showsRecord;
  const findingsStatus = !readsFindings
    ? undefined
    : // Findings loaded before a failed refresh are still worth showing.
      findings.data !== undefined
      ? fieldFindingsStatuses.loaded
      : findings.isError
        ? fieldFindingsStatuses.failed
        : fieldFindingsStatuses.loading;
  return useMemo(
    () => ({
      attributes,
      connections,
      contextCode,
      contextId,
      findings:
        findingsStatus === fieldFindingsStatuses.loaded
          ? findings.data
          : undefined,
      findingsStatus,
      findingsError: readsFindings ? findings.error : undefined,
      showsValues: showsRecord,
    }),
    [
      attributes,
      connections,
      contextCode,
      contextId,
      findings.data,
      findings.error,
      findingsStatus,
      readsFindings,
      showsRecord,
    ],
  );
};
