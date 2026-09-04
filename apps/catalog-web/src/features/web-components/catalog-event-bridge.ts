import {
  catalogEventNames,
  type NavigateDetail,
  navigateDetailSchema,
  type NotifyDetail,
  notifyDetailSchema,
  type RefreshEntityDetail,
  refreshEntityDetailSchema,
} from './catalog-events';

export type CatalogEventBridgeOptions = {
  host: EventTarget;
  onRefreshEntity: (detail: RefreshEntityDetail) => void;
  onNavigate: (detail: NavigateDetail) => void;
  onNotify: (detail: NotifyDetail) => void;
};

const validatedHandler =
  <T>(
    schema: { safeParse: (detail: unknown) => { success: boolean; data?: T } },
    callback: (detail: T) => void,
  ) =>
  (event: Event) => {
    const result = schema.safeParse((event as CustomEvent<unknown>).detail);
    if (!result.success) return;

    callback(result.data!);
    event.preventDefault();
  };

/**
 * Connects a custom-element host to the three component-to-host catalog events.
 * Invalid details are ignored. Call `dispose` when the host is unmounted.
 */
export const installCatalogEventBridge = ({
  host,
  onRefreshEntity,
  onNavigate,
  onNotify,
}: CatalogEventBridgeOptions) => {
  const refreshEntity = validatedHandler(
    refreshEntityDetailSchema,
    onRefreshEntity,
  );
  const navigate = validatedHandler(navigateDetailSchema, onNavigate);
  const notify = validatedHandler(notifyDetailSchema, onNotify);

  host.addEventListener(catalogEventNames.refreshEntity, refreshEntity);
  host.addEventListener(catalogEventNames.navigate, navigate);
  host.addEventListener(catalogEventNames.notify, notify);

  return () => {
    host.removeEventListener(catalogEventNames.refreshEntity, refreshEntity);
    host.removeEventListener(catalogEventNames.navigate, navigate);
    host.removeEventListener(catalogEventNames.notify, notify);
  };
};
