import type { TFunction } from 'i18next';
import { z } from 'zod';
import { errorMessage } from '../../api/errors';
import { ApiRequestError } from '../../api/request';
import {
  RULE_DRY_RUN_REQUIRED,
  RULE_HAS_EXISTING_VIOLATIONS,
} from './constants';

const truncatedDetailsSchema = z.object({
  truncated: z.literal(true),
  existing_violations: z.number(),
});

/** A dry run that reached the record limit, which running again cannot fix. */
const truncatedDryRun = (error: unknown) =>
  error instanceof ApiRequestError && error.code === RULE_DRY_RUN_REQUIRED
    ? truncatedDetailsSchema.safeParse(error.details).data
    : undefined;

/** Whether the server would take this change if existing violations are accepted. */
export const canAcceptExistingViolations = (error: unknown) =>
  (error instanceof ApiRequestError &&
    error.code === RULE_HAS_EXISTING_VIOLATIONS) ||
  truncatedDryRun(error) !== undefined;

/** Why a dry run holds back an enforcing rule. */
export const dryRunGateMessage = (t: TFunction, error: Error) => {
  const truncated = truncatedDryRun(error);
  return truncated
    ? t('rules.dryRunTruncated', {
        existing_violations: truncated.existing_violations,
      })
    : errorMessage(error);
};
