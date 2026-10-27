import { createContext, useContext, useMemo } from 'react';
import { findingSeverities } from '../rules/constants';
import { findingEvidenceSchema, type Finding } from '../rules/schemas';
import type { Attribute } from './api';
import type { AttributeConnection } from './attributeConnections';

/** Where loading a record's findings for its field diagnostics stands. */
export const fieldFindingsStatuses = {
  loading: 'loading',
  failed: 'failed',
  loaded: 'loaded',
} as const;

export type FieldFindingsStatus =
  (typeof fieldFindingsStatuses)[keyof typeof fieldFindingsStatuses];

/** What a record page knows about its fields beyond their values. */
export type FieldDiagnostics = {
  /** The context the page shows, by code. */
  contextCode?: string;
  contextId?: string;
  /**
   * The record's unresolved data quality findings, or `undefined` when the
   * user cannot read them or they have not loaded.
   */
  findings?: readonly Finding[];
  /**
   * Why `findings` is missing although the user can read them: they are
   * still loading, or loading them failed with `findingsError`.
   */
  findingsStatus?: FieldFindingsStatus;
  findingsError?: Error | null;
  /** Every attribute of the record, to name connected fields. */
  attributes?: readonly Attribute[];
  /** What the blueprint and its rules tie attributes to. */
  connections?: readonly AttributeConnection[];
  /**
   * Whether fields show a saved record's resolved values, so it can be said
   * where each comes from; a form being filled in has none.
   */
  showsValues?: boolean;
};

/**
 * Supplied by the record page and the forms that edit a blueprint's fields.
 * Fields elsewhere, such as related record previews, render without
 * diagnostics.
 */
export const FieldDiagnosticsContext = createContext<FieldDiagnostics | null>(
  null,
);

export const useFieldDiagnostics = () => useContext(FieldDiagnosticsContext);

/**
 * Findings about one attribute that apply to the context shown. A finding
 * for one context stays there; a record-wide one lists the contexts it
 * fails in.
 */
export const fieldFindings = (
  findings: readonly Finding[],
  attributeCode: string,
  {
    contextCode,
    contextId,
  }: Pick<FieldDiagnostics, 'contextCode' | 'contextId'>,
) =>
  findings.filter((finding) => {
    const evidence = findingEvidenceSchema.safeParse(finding.evidence);
    if (!evidence.success || evidence.data.attribute_code !== attributeCode)
      return false;
    if (finding.context_id) return finding.context_id === contextId;
    const contexts = evidence.data.contexts;
    return (
      !contexts || (contextCode !== undefined && contexts.includes(contextCode))
    );
  });

const severityRank: readonly string[] = [
  findingSeverities.info,
  findingSeverities.warning,
  findingSeverities.error,
  findingSeverities.critical,
];

/** The most severe of the findings' severities. */
export const worstSeverity = (findings: readonly Finding[]) =>
  findings.reduce<string | undefined>(
    (worst, finding) =>
      worst === undefined ||
      severityRank.indexOf(finding.severity) > severityRank.indexOf(worst)
        ? finding.severity
        : worst,
    undefined,
  );

/**
 * The findings about one field in the context shown, or `undefined` outside
 * the record page or when the user cannot read findings.
 */
export const useFieldFindings = (attributeCode: string) => {
  const diagnostics = useFieldDiagnostics();
  return useMemo(
    () =>
      diagnostics?.findings &&
      fieldFindings(diagnostics.findings, attributeCode, diagnostics),
    [attributeCode, diagnostics],
  );
};
