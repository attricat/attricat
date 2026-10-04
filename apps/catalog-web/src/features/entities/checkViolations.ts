import type { CheckViolation } from '../../api/checkViolations';
import type { EntityPublicationReadiness } from './schemas';

/**
 * Places each violation's message on the form fields it involves. Violations
 * that name no field shown in the form are returned for a summary instead.
 * Pass `text` to word each message, for example with its failing contexts.
 */
export const violationFieldErrors = (
  violations: readonly CheckViolation[],
  fieldCodes: readonly string[],
  text: (violation: CheckViolation) => string = (violation) =>
    violation.message,
) => {
  const fields = new Set(fieldCodes);
  const fieldErrors: Record<string, string> = {};
  const unplaced: CheckViolation[] = [];
  for (const violation of violations) {
    const involved = violation.attributes.filter((code) => fields.has(code));
    if (involved.length === 0) {
      unplaced.push(violation);
      continue;
    }
    const message = text(violation);
    for (const code of involved)
      fieldErrors[code] = fieldErrors[code]
        ? `${fieldErrors[code]} ${message}`
        : message;
  }
  return { fieldErrors, unplaced };
};

/** Violation messages joined into one run of text. */
export const violationsText = (violations: readonly CheckViolation[]) =>
  violations.map((violation) => violation.message).join(' ');

/** The failing checks that keep an entity from one channel, as one sentence. */
export const publicationReadinessText = (
  readiness: EntityPublicationReadiness,
) => violationsText(readiness.violations);

/** The evaluated readiness of the channel for `contextId`, if any. */
export const readinessForContext = (
  readiness: readonly EntityPublicationReadiness[] | undefined,
  contextId: string | null | undefined,
) =>
  contextId
    ? readiness?.find((item) => item.context_id === contextId)
    : undefined;
