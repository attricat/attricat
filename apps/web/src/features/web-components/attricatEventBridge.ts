import {
  attricatEventNames,
  type NavigateDetail,
  navigateDetailSchema,
  type NotifyDetail,
  notifyDetailSchema,
  type RefreshRecordDetail,
  refreshRecordDetailSchema,
} from './attricatEvents';

export type AttricatEventBridgeOptions = {
  host: EventTarget;
  onRefreshRecord: (detail: RefreshRecordDetail) => void;
  onNavigate: (detail: NavigateDetail) => void;
  onNotify: (detail: NotifyDetail) => void;
};

const validatedHandler =
  <T>(
    schema: {
      safeParse: (
        detail: unknown,
      ) => { success: true; data: T } | { success: false };
    },
    callback: (detail: T) => void,
  ) =>
  (event: Event) => {
    const result = schema.safeParse((event as CustomEvent<unknown>).detail);
    if (!result.success) return;

    callback(result.data);
    event.preventDefault();
  };

/**
 * Connects a custom-element host to the three component-to-host catalog events.
 * Invalid details are ignored. Call `dispose` when the host is unmounted.
 */
export const installAttricatEventBridge = ({
  host,
  onRefreshRecord,
  onNavigate,
  onNotify,
}: AttricatEventBridgeOptions) => {
  const refreshRecord = validatedHandler(
    refreshRecordDetailSchema,
    onRefreshRecord,
  );
  const navigate = validatedHandler(navigateDetailSchema, onNavigate);
  const notify = validatedHandler(notifyDetailSchema, onNotify);

  host.addEventListener(attricatEventNames.refreshRecord, refreshRecord);
  host.addEventListener(attricatEventNames.navigate, navigate);
  host.addEventListener(attricatEventNames.notify, notify);

  return () => {
    host.removeEventListener(attricatEventNames.refreshRecord, refreshRecord);
    host.removeEventListener(attricatEventNames.navigate, navigate);
    host.removeEventListener(attricatEventNames.notify, notify);
  };
};
