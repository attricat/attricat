import { z } from 'zod';
import type {
  Attribute,
  FormAttributeValue,
  StatusTransitionOptions,
} from './api';

export const STATUS_SCHEMA_KEY = 'x-attricat-status';
const codeSchema = z
  .string()
  .min(1)
  .max(128)
  .regex(/^[A-Za-z0-9_-]+$/);
export const statusConfigurationSchema = z
  .object({
    version: z.literal(1),
    options: z
      .array(
        z
          .object({
            code: codeSchema,
            label: z.string().trim().min(1).max(200),
            tone: z
              .enum(['default', 'success', 'warning', 'error', 'info'])
              .optional(),
          })
          .strict(),
      )
      .min(1)
      .max(100),
    transitions: z
      .array(
        z
          .object({
            from: codeSchema.nullable(),
            to: codeSchema.nullable(),
          })
          .strict(),
      )
      .max(10000)
      .optional(),
  })
  .strict();
export type StatusConfiguration = z.infer<typeof statusConfigurationSchema>;

export const statusConfiguration = (
  attribute: Attribute,
): StatusConfiguration | undefined => {
  if (
    attribute.value_type !== 'string' ||
    !attribute.value_schema ||
    typeof attribute.value_schema !== 'object'
  )
    return undefined;
  const result = statusConfigurationSchema.safeParse(
    attribute.value_schema[STATUS_SCHEMA_KEY],
  );
  return result.success ? result.data : undefined;
};

export const statusTransitionAllowed = (
  config: StatusConfiguration,
  before: string | null,
  after: string | null,
) => {
  if (before === after) return true;
  if (after !== null && !config.options.some((option) => option.code === after))
    return false;
  if (config.transitions)
    return config.transitions.some(
      (edge) => edge.from === before && edge.to === after,
    );
  return after !== null;
};

export type StatusDestination = StatusTransitionOptions['destinations'][number];

/**
 * The server-evaluated destination that blocks moving from `before` to
 * `after`, if any. Without server destinations (for example, an unsaved
 * entity) only the transition graph applies.
 */
export const blockedStatusDestination = (
  destinations: readonly StatusDestination[] | undefined,
  before: string | null,
  after: string | null,
): StatusDestination | undefined => {
  if (!destinations || after === null || after === before) return undefined;
  const destination = destinations.find((item) => item.to === after);
  return destination && !destination.allowed ? destination : undefined;
};

/** Server destinations for one status attribute from the form's baseline. */
export const statusDestinationsFor = (
  options: readonly StatusTransitionOptions[] | undefined,
  attributeCode: string,
  baseline: string | null,
): StatusDestination[] | undefined => {
  const match = options?.find((item) => item.attribute_code === attributeCode);
  // A response for another saved state (for example, a concurrent edit) does
  // not describe the transitions this form will submit.
  return match && match.current === baseline ? match.destinations : undefined;
};

export const statusParentContexts = (
  contexts: readonly { id: string; parent_id?: string | null }[],
  contextId: string | null,
): string[] => {
  const path: string[] = [];
  const visited = new Set([contextId]);
  let parent = contexts.find((context) => context.id === contextId)?.parent_id;
  while (parent && !visited.has(parent)) {
    visited.add(parent);
    path.push(parent);
    parent = contexts.find((context) => context.id === parent)?.parent_id;
  }
  return path;
};

export const savedStatusValue = (
  attribute: Attribute,
  values: readonly FormAttributeValue[],
  contextIds: readonly (string | null)[],
): string | null => {
  for (const contextId of contextIds) {
    const value = values.find(
      (item) =>
        item.kind === 'scalar' &&
        item.attribute_code === attribute.code &&
        item.context_id === contextId,
    );
    if (value?.kind === 'scalar' && typeof value.value === 'string')
      return value.value;
  }
  return null;
};

/**
 * The saved status a transition starts from (`current`) and the value inherited
 * from parent contexts that applies when the local value is removed.
 */
export const savedStatusState = (
  attribute: Attribute,
  values: readonly FormAttributeValue[],
  contextId: string | null,
  parentContextIds: readonly string[],
) => {
  const parents = attribute.context_fallback === 'none' ? [] : parentContextIds;
  return {
    current: savedStatusValue(attribute, values, [contextId, ...parents]),
    inherited: savedStatusValue(attribute, values, parents),
  };
};

export const statusLabel = (
  attribute: Attribute,
  value: unknown,
): string | undefined =>
  statusConfiguration(attribute)?.options.find(
    (option) => option.code === value,
  )?.label;
