import { z } from 'zod';

export const catalogEventNames = {
  entityUpdated: 'catalog:entity-updated.v1',
  contextChanged: 'catalog:context-changed.v1',
  refreshEntity: 'catalog:refresh-entity.v1',
  navigate: 'catalog:navigate.v1',
  notify: 'catalog:notify.v1',
} as const;

const correlationIdSchema = z.uuid().optional();
const changeHintSchema = z.enum([
  'entity',
  'attribute_values',
  'relationships',
  'blueprint',
]);

/** Host-to-component details. They deliberately exclude outbox envelopes and payloads. */
export const entityUpdatedDetailSchema = z
  .object({
    entity_id: z.uuid(),
    change_hints: z.array(changeHintSchema).max(4),
    correlation_id: correlationIdSchema,
  })
  .strict();

/** Host-to-component details. Components re-fetch any authorized context data. */
export const contextChangedDetailSchema = z
  .object({
    context_id: z.uuid(),
    correlation_id: correlationIdSchema,
  })
  .strict();

/** Component-to-host request to re-fetch an entity's authorized current data. */
export const refreshEntityDetailSchema = z
  .object({
    entity_id: z.uuid(),
    change_hints: z.array(changeHintSchema).max(4).optional(),
    correlation_id: correlationIdSchema,
  })
  .strict();

/** Component-to-host navigation request. The host owns the actual route. */
export const navigateDetailSchema = z
  .object({
    entity_id: z.uuid(),
    correlation_id: correlationIdSchema,
  })
  .strict();

/** Component-to-host request for a bounded, user-facing notification. */
export const notifyDetailSchema = z
  .object({
    message: z.string().trim().min(1).max(512),
    severity: z.enum(['success', 'info', 'warning', 'error']).default('info'),
    correlation_id: correlationIdSchema,
  })
  .strict();

export type EntityUpdatedDetail = z.infer<typeof entityUpdatedDetailSchema>;
export type ContextChangedDetail = z.infer<typeof contextChangedDetailSchema>;
export type RefreshEntityDetail = z.infer<typeof refreshEntityDetailSchema>;
export type NavigateDetail = z.infer<typeof navigateDetailSchema>;
export type NotifyDetail = z.infer<typeof notifyDetailSchema>;

type CatalogEventName =
  (typeof catalogEventNames)[keyof typeof catalogEventNames];

const dispatch = <T>(
  target: EventTarget,
  type: CatalogEventName,
  detail: T,
  bubbles: boolean,
) =>
  target.dispatchEvent(
    new CustomEvent(type, { bubbles, composed: bubbles, detail }),
  );

export const dispatchEntityUpdated = (
  target: EventTarget,
  detail: EntityUpdatedDetail,
) =>
  dispatch(
    target,
    catalogEventNames.entityUpdated,
    entityUpdatedDetailSchema.parse(detail),
    false,
  );

export const dispatchContextChanged = (
  target: EventTarget,
  detail: ContextChangedDetail,
) =>
  dispatch(
    target,
    catalogEventNames.contextChanged,
    contextChangedDetailSchema.parse(detail),
    false,
  );

export const requestEntityRefresh = (
  target: EventTarget,
  detail: RefreshEntityDetail,
) =>
  dispatch(
    target,
    catalogEventNames.refreshEntity,
    refreshEntityDetailSchema.parse(detail),
    true,
  );

export const requestNavigation = (
  target: EventTarget,
  detail: NavigateDetail,
) =>
  dispatch(
    target,
    catalogEventNames.navigate,
    navigateDetailSchema.parse(detail),
    true,
  );

export const requestNotification = (
  target: EventTarget,
  detail: NotifyDetail,
) =>
  dispatch(
    target,
    catalogEventNames.notify,
    notifyDetailSchema.parse(detail),
    true,
  );
