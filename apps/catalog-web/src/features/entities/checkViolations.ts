import { ApiRequestError } from '../../api/request';
import {
  checkViolationDetailsSchema,
  type CheckViolation,
  type EntityPublicationReadiness,
} from './schemas';

/** API error codes whose `details.violations` list failed declarative checks. */
export const checkViolationErrorCodes = {
  entityCheckFailed: 'entity_check_failed',
  transitionConditionsUnmet: 'transition_conditions_unmet',
  ruleViolation: 'rule_violation',
  publicationChecksFailed: 'publication_checks_failed',
} as const;

const violationCodes = new Set<string>(Object.values(checkViolationErrorCodes));

export type CheckViolationError = {
  message: string;
  violations: CheckViolation[];
  /** Publication channel code for `publication_checks_failed`. */
  context?: string;
};

/** Reads structured check violations from an API error, if it carries them. */
export const checkViolationError = (
  error: unknown,
): CheckViolationError | undefined => {
  if (
    !(error instanceof ApiRequestError) ||
    !error.code ||
    !violationCodes.has(error.code)
  )
    return undefined;
  const details = checkViolationDetailsSchema.safeParse(error.details);
  if (!details.success) return undefined;
  return {
    message: error.message,
    violations: details.data.violations,
    ...(details.data.context ? { context: details.data.context } : {}),
  };
};

/**
 * Places each violation's message on the form fields it involves. Violations
 * that name no field shown in the form are returned for a summary instead.
 */
export const violationFieldErrors = (
  violations: readonly CheckViolation[],
  fieldCodes: readonly string[],
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
    for (const code of involved)
      fieldErrors[code] = fieldErrors[code]
        ? `${fieldErrors[code]} ${violation.message}`
        : violation.message;
  }
  return { fieldErrors, unplaced };
};

/** The failing checks that keep an entity from one channel, as one sentence. */
export const publicationReadinessText = (
  readiness: EntityPublicationReadiness,
) => readiness.violations.map((violation) => violation.message).join(' ');
