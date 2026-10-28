import { z } from 'zod';
import type { Rule } from './api';
import { findingSeverities } from './constants';

/** The parts of a stored plan the app shows beside a rule. */
const plannedSchema = z.object({
  severity: z.enum(findingSeverities).optional().catch(undefined),
  description: z.string().nullish().catch(undefined),
  predicate: z.looseObject({ type: z.string() }).optional().catch(undefined),
});

/** The severity of a rule's current revision, when its plan names one. */
export const ruleSeverity = (rule: Rule) =>
  plannedSchema.safeParse(rule.compiled_plan).data?.severity;

/** What a rule's current revision says it is for, when it says. */
export const ruleDescription = (rule: Rule) =>
  plannedSchema.safeParse(rule.compiled_plan).data?.description ?? undefined;

/** What a rule's current revision checks, as its plan stores it. */
export const rulePredicate = (rule: Rule) =>
  plannedSchema.safeParse(rule.compiled_plan).data?.predicate;
