import { z } from 'zod';
import { ApiRequestError } from './request';
import type { ApiErrorCode } from './errors';

/** One failed declarative check, condition or rule (`error.details.violations`). */
export const checkViolationSchema = z.object({
  source: z.string(),
  code: z.string(),
  message: z.string(),
  contexts: z.array(z.string()).default([]),
  attributes: z.array(z.string()).default([]),
  severity: z.string().optional(),
  transition: z
    .object({
      attribute_code: z.string(),
      from: z.string().nullable(),
      to: z.string().nullable(),
    })
    .optional(),
  evidence: z.unknown().optional(),
});
const checkViolationDetailsSchema = z.object({
  violations: z.array(checkViolationSchema),
});
export type CheckViolation = z.infer<typeof checkViolationSchema>;

/** API error codes whose `details.violations` list failed declarative checks. */
export const checkViolationErrorCodes = {
  recordCheckFailed: 'record_check_failed',
  transitionConditionsUnmet: 'transition_conditions_unmet',
  ruleViolation: 'rule_violation',
  publicationChecksFailed: 'publication_checks_failed',
} as const satisfies Record<string, ApiErrorCode>;

const violationCodes = new Set<string>(Object.values(checkViolationErrorCodes));

type CheckViolationError = {
  violations: CheckViolation[];
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
  return { violations: details.data.violations };
};
