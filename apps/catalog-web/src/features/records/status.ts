import { z } from 'zod';
import { lexiconText } from '../lexicon/lexicon';
import type {
  Attribute,
  FormAttributeValue,
  StatusTransitionAccess,
} from './api';

export const STATUS_SCHEMA_KEY = 'x-attricat-status';
const codeSchema = z
  .string()
  .min(1)
  .max(128)
  .regex(/^[A-Za-z0-9_-]+$/);
const coverageSchema = z.union([
  z.literal('all'),
  z.array(z.string().min(1).max(257)).min(1).max(500),
]);
/** Attributes locked by a status or covered by its approval. */
export type StatusCoverage = z.infer<typeof coverageSchema>;
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
            lock: coverageSchema.optional(),
            approval: z
              .object({ covers: coverageSchema, void_to: codeSchema })
              .strict()
              .optional(),
            retention_days: z.number().int().min(1).max(36600).optional(),
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
            code: codeSchema.optional(),
            permission: z.string().max(128).optional(),
            roles: z.array(z.string().min(1).max(64)).min(1).max(20).optional(),
            separate_from: z.array(codeSchema).min(1).max(20).optional(),
            conditions: z
              .array(
                z
                  .object({
                    code: codeSchema,
                    message: z.string().min(1).max(500).optional(),
                    // Match the public status metadata contract. Predicate
                    // semantics and transition access are checked by the server.
                    predicate: z.object({ type: z.string() }).passthrough(),
                  })
                  .strict(),
              )
              .max(16)
              .optional(),
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

/**
 * Why a status selection is refused. `undeclared` edges are not in the
 * transition graph; `access` edges are declared but the server refuses them
 * to the caller (permission, role, separation of duties or conditions).
 */
export type StatusTransitionDenial =
  { kind: 'undeclared' } | { kind: 'access'; access: StatusTransitionAccess };

/**
 * Why moving the saved `baseline` status to `selected` is refused, or
 * `undefined` when it is allowed. An empty selection removes the local value,
 * so the transition targets the `inherited` status instead.
 */
export const statusTransitionDenial = (
  config: StatusConfiguration,
  {
    attributeCode,
    baseline,
    inherited,
    selected,
    transitions = [],
  }: {
    attributeCode: string;
    baseline: string | null;
    inherited: string | null;
    selected: string;
    /** Server-evaluated access to edges leaving the saved status. */
    transitions?: readonly StatusTransitionAccess[];
  },
): StatusTransitionDenial | undefined => {
  const target = selected || inherited;
  if (!statusTransitionAllowed(config, baseline, target))
    return { kind: 'undeclared' };
  if (target === baseline) return undefined;
  const access = transitions.find(
    (edge) =>
      edge.attribute_code === attributeCode &&
      edge.to === target &&
      !edge.allowed,
  );
  return access && { kind: 'access', access };
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
/** `all` never covers the status attribute that declares it. */
export const statusCovers = (
  coverage: StatusCoverage,
  attributeCode: string,
  statusCode: string,
) =>
  coverage === 'all'
    ? attributeCode !== statusCode
    : coverage.includes(attributeCode);

/**
 * Attribute codes made read-only by the saved status of each status attribute
 * in this context, mapped to that status label. The server enforces the same
 * lock on every write path; this only explains it in the form.
 */
export const statusLocks = (
  attributes: readonly Attribute[],
  values: readonly FormAttributeValue[],
  contextId: string | null,
  parentContextIds: readonly string[],
): Record<string, string> => {
  const locks: Record<string, string> = {};
  for (const statusAttribute of attributes) {
    const config = statusConfiguration(statusAttribute);
    if (!config) continue;
    const { current } = savedStatusState(
      statusAttribute,
      values,
      contextId,
      parentContextIds,
    );
    const option = config.options.find((item) => item.code === current);
    if (!option?.lock) continue;
    for (const attribute of attributes)
      if (statusCovers(option.lock, attribute.code, statusAttribute.code))
        locks[attribute.code] ??= statusOptionLabel(option);
  }
  return locks;
};
