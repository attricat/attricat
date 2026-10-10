import { z } from 'zod';
import {
  catalogEventNames,
  defaultNotificationSeverity,
  recordChangeHints,
  maximumNotificationMessageLength,
  notificationSeverities,
} from './constants';

export { catalogEventNames } from './constants';

const correlationIdSchema = z.uuid().optional();
const changeHintSchema = z.enum(recordChangeHints);
const changeHintsSchema = z
  .array(changeHintSchema)
  .max(recordChangeHints.length);

/** Host-to-component details. They deliberately exclude outbox envelopes and payloads. */
export const recordUpdatedDetailSchema = z
  .object({
    record_id: z.uuid(),
    change_hints: changeHintsSchema,
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

/** Component-to-host request to re-fetch a record's authorized current data. */
export const refreshRecordDetailSchema = z
  .object({
    record_id: z.uuid(),
    change_hints: changeHintsSchema.optional(),
    correlation_id: correlationIdSchema,
  })
  .strict();

/** Component-to-host navigation request. The host owns the actual route. */
export const navigateDetailSchema = z
  .object({
    record_id: z.uuid(),
    correlation_id: correlationIdSchema,
  })
  .strict();

/** Component-to-host request for a bounded, user-facing notification. */
export const notifyDetailSchema = z
  .object({
    message: z.string().trim().min(1).max(maximumNotificationMessageLength),
    severity: z
      .enum(notificationSeverities)
      .default(defaultNotificationSeverity),
    correlation_id: correlationIdSchema,
  })
  .strict();

export type RecordUpdatedDetail = z.infer<typeof recordUpdatedDetailSchema>;
export type ContextChangedDetail = z.infer<typeof contextChangedDetailSchema>;
export type RefreshRecordDetail = z.infer<typeof refreshRecordDetailSchema>;
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

export const dispatchRecordUpdated = (
  target: EventTarget,
  detail: RecordUpdatedDetail,
) =>
  dispatch(
    target,
    catalogEventNames.recordUpdated,
    recordUpdatedDetailSchema.parse(detail),
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

export const requestRecordRefresh = (
  target: EventTarget,
  detail: RefreshRecordDetail,
) =>
  dispatch(
    target,
    catalogEventNames.refreshRecord,
    refreshRecordDetailSchema.parse(detail),
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
