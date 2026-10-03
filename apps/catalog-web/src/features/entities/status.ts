import { z } from 'zod';
import { lexiconText } from '../lexicon/lexicon';
import type { Attribute, FormAttributeValue } from './api';

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

type StatusOption = StatusConfiguration['options'][number];

/**
 * An option's label for the UI language. Labels may reference the workspace
 * lexicon (`{{key}}`) and fall back like other catalog-defined labels.
 */
export const statusOptionLabel = (option: StatusOption): string =>
  lexiconText(option.label);

/** The localized label of a configured status code, if `value` is one. */
export const statusCodeLabel = (
  config: StatusConfiguration,
  value: unknown,
): string | undefined => {
  const option = config.options.find((item) => item.code === value);
  return option && statusOptionLabel(option);
};

export const statusLabel = (
  attribute: Attribute,
  value: unknown,
): string | undefined => {
  const config = statusConfiguration(attribute);
  return config && statusCodeLabel(config, value);
};
