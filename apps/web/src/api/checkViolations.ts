import { z } from 'zod';
import { ApiRequestError } from './request';

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
export const checkViolationDetailsSchema = z.object({
  violations: z.array(checkViolationSchema),
  context: z.string().optional(),
});
export type CheckViolation = z.infer<typeof checkViolationSchema>;

/** API error codes whose `details.violations` list failed declarative checks. */
export const checkViolationErrorCodes = {
  recordCheckFailed: 'record_check_failed',
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
