import type { ViewNode } from '../records/api';
import {
  fieldFindings,
  worstSeverity,
  type FieldDiagnostics,
} from '../records/fieldDiagnostics';
import { findingSeverityColor } from '../rules/findingSeverity';
import { nodesPlacedFields } from './viewFieldComponents';

/** Why a tab or accordion section is marked. */
const layoutMarkerReasons = {
  errors: 'errors',
  findings: 'findings',
} as const;

/** What a marked label tells assistive technology, by reason. */
export const layoutMarkerDescriptions = {
  [layoutMarkerReasons.errors]: 'views.tabHasErrors',
  [layoutMarkerReasons.findings]: 'views.tabHasFindings',
} as const;

type LayoutMarker = {
  reason: (typeof layoutMarkerReasons)[keyof typeof layoutMarkerReasons];
  /** The palette colour of the label and its icon. */
  color: string;
};

/**
 * How a tab or accordion section points to the fields it hides: fields with
 * errors first, then the most severe data quality finding about any field
 * in it, or `undefined` when nothing in it needs attention.
 */
export const layoutMarker = (
  nodes: readonly ViewNode[],
  invalidFields?: ReadonlySet<string>,
  diagnostics?: FieldDiagnostics | null,
): LayoutMarker | undefined => {
  const fields = nodesPlacedFields(nodes);
  if ([...fields].some((code) => invalidFields?.has(code)))
    return { reason: layoutMarkerReasons.errors, color: 'error.main' };
  if (!diagnostics?.findings?.length) return undefined;
  const { findings } = diagnostics;
  const severity = worstSeverity(
    [...fields].flatMap((code) => fieldFindings(findings, code, diagnostics)),
  );
  return severity
    ? {
        reason: layoutMarkerReasons.findings,
        color: `${findingSeverityColor(severity)}.main`,
      }
    : undefined;
};
