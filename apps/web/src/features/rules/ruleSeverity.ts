import { z } from 'zod';
import type { Rule } from './api';
import { findingSeverities } from './constants';

const plannedSeveritySchema = z.object({
  severity: z.enum(findingSeverities),
});

/** The severity of a rule's current revision, when its plan names one. */
export const ruleSeverity = (rule: Rule) =>
  plannedSeveritySchema.safeParse(rule.compiled_plan).data?.severity;
