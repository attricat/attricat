import {
  catalogEventNames,
  type NavigateDetail,
  navigateDetailSchema,
  type NotifyDetail,
  notifyDetailSchema,
  type RefreshRecordDetail,
  refreshRecordDetailSchema,
} from './catalogEvents';

export type CatalogEventBridgeOptions = {
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
export const installCatalogEventBridge = ({
  host,
  onRefreshRecord,
  onNavigate,
  onNotify,
}: CatalogEventBridgeOptions) => {
  const refreshRecord = validatedHandler(
    refreshRecordDetailSchema,
    onRefreshRecord,
  );
  const navigate = validatedHandler(navigateDetailSchema, onNavigate);
  const notify = validatedHandler(notifyDetailSchema, onNotify);

  host.addEventListener(catalogEventNames.refreshRecord, refreshRecord);
  host.addEventListener(catalogEventNames.navigate, navigate);
  host.addEventListener(catalogEventNames.notify, notify);

  return () => {
    host.removeEventListener(catalogEventNames.refreshRecord, refreshRecord);
    host.removeEventListener(catalogEventNames.navigate, navigate);
    host.removeEventListener(catalogEventNames.notify, notify);
  };
};
