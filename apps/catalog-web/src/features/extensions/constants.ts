export const defaultExtensionFrameHeight = 48;
export const extensionStartTimeout = 10_000;
export const extensionRuntimeRefetchInterval = 15_000;
export const maximumExtensionFrameHeight = 2_048;
export const maximumExtensionStorageKeyBytes = 256;
export const maximumExtensionRequestBytes = 65_536;
export const maximumExtensionResponseBytes = 1_048_576;

export const minimumContributionKeyLength = 3;
export const maximumContributionKeyLength = 256;
export const contributionKeyPattern = /^[A-Za-z0-9._-]+:[A-Za-z0-9._-]+$/;
export const maximumExtensionCommandIdLength = 128;
export const maximumExtensionStorageListLimit = 100;
export const supportedWorkspaceLayoutVersion = 1;
export const supportedOutletContextVersion = 1;

/** Host-owned insertion points an extension contribution may target. */
export const extensionOutletNames = [
  'navigation',
  'entity_preview_panel',
  'blueprint_attribute_configuration',
  'entity_attribute_decoration',
  'entity_action',
  'explorer_row_action',
  'explorer_table_cell',
  'blueprint_detail_panel',
  'explorer_action',
  'explorer_bulk_action',
  'entity_header_action',
  'entity_attribute_panel',
  'blueprint_panel',
  'blueprint_publish_check',
  'file_panel',
  'audit_event_panel',
  'data_health_card',
] as const;
export const navigationOutlet = 'navigation';

export const contributionKinds = [
  'route',
  'navigation',
  'embedded',
  'action',
  'panel',
] as const;
export const navigationGroups = ['promoted', 'grouped'] as const;
export const promotedNavigationGroup = 'promoted';

export const installationStates = [
  'disabled',
  'enabled',
  'quarantined',
] as const;
export type InstallationState = (typeof installationStates)[number];

export const grantKinds = [
  'capability',
  'host_permission',
  'event_publish',
  'event_subscribe',
] as const;
export type GrantKind = (typeof grantKinds)[number];

export const lifecycleOperations = [
  'install',
  'configure',
  'grant',
  'revoke',
  'enable',
  'disable',
  'upgrade',
  'quarantine',
  'remove',
] as const;
export type LifecycleOperation = (typeof lifecycleOperations)[number];

export const manualQuarantineDiagnosticCode = 'manual_quarantine';
export const extensionArchiveAccept = '.tar.zst,application/zstd';
export const githubRepositoryPrefix = /^github:/;
export const releaseTagSeparator = '@';

/** Capabilities a contribution must hold before the host brokers a request. */
export const extensionCapabilities = {
  catalogRead: 'catalog.read',
  commands: 'client.commands',
  events: 'client.events',
  navigation: 'client.navigation',
  notification: 'client.notification',
  refresh: 'client.refresh',
  storage: 'storage.extension',
} as const;

/** Versioned host/frame MessageChannel protocol message types. */
export const extensionMessageTypes = {
  contextChanged: 'catalog:context-changed.v1',
  contextUpdate: 'catalog:context-update.v1',
  error: 'catalog:error.v1',
  init: 'catalog:init.v1',
  ready: 'catalog:ready.v1',
  request: 'catalog:request.v1',
  resize: 'catalog:resize.v1',
  response: 'catalog:response.v1',
  shutdown: 'catalog:shutdown.v1',
  themeChanged: 'catalog:theme-changed.v1',
  themeUpdate: 'catalog:theme-update.v1',
} as const;

/** Broker methods an extension frame can call through its MessagePort. */
export const extensionBrokerMethods = {
  catalogRead: 'catalog.read',
  command: 'command',
  navigate: 'navigate',
  notify: 'notify',
  refresh: 'refresh',
} as const;
export const extensionStorageMethodPrefix = 'storage.';
export const extensionStorageOperations = [
  'get',
  'set',
  'delete',
  'list',
] as const;

/**
 * Protocol errors are returned to or thrown inside extension code; they are not
 * rendered by the host and therefore stay untranslated.
 */
export const extensionProtocolErrors = {
  blueprintRevisionOutsideContext:
    'Blueprint revision is outside this outlet context',
  hostRequestCancelled: 'Host request cancelled',
  hostRequestFailed: 'Host request failed',
  missingMountExport: 'Extension must export mount(root, catalog)',
  catalogResponseTooLarge: 'Catalog response is too large',
  commandPayloadTooLarge: 'Command payload is too large',
  invalidStorageKey: 'Invalid storage key',
  refreshDenied: 'Refresh denied',
  requestDenied: 'Request denied',
  storageRequestDenied: 'Storage request denied',
  storageRequestTooLarge: 'Storage request is too large',
  storageValueTooLarge: 'Storage value is too large',
} as const;

/** Validation messages for workspace layout drafts; the page shows its own copy. */
export const layoutValidationMessages = {
  duplicateKeys: 'Contribution keys must be unique',
  hiddenAndOrdered: 'A contribution cannot be ordered and hidden',
  hiddenAndPromoted: 'A contribution cannot be hidden and promoted',
  invalidOutlet: 'Invalid outlet layout',
  multipleOutlets: 'A contribution can be configured in only one outlet',
} as const;

// Components may read current entity data or the exact blueprint revision named
// by the blueprint-configuration outlet. They cannot supply methods, query
// strings, arbitrary URLs, or credentials.
export const catalogReadPathPattern =
  /^\/api\/(?:entities|v1\/entities\/[0-9a-f-]{36}|blueprints\/[0-9a-f-]{36}\/versions\/[1-9][0-9]*)$/;
export const blueprintRevisionPathPattern =
  /^\/api\/blueprints\/([0-9a-f-]{36})\/versions\/([1-9][0-9]*)$/;

/** C0 and C1 control characters are rejected in extension storage keys. */
export const controlCharacterRanges = {
  c0End: 0x1f,
  c1Start: 0x7f,
  c1End: 0x9f,
} as const;

export const extensionCardMinWidth = 190;
export const extensionPopoverWidth = 480;
export const extensionLoadingIndicatorSize = 20;
export const extensionFrameSkeletonLines = [
  { height: 20, width: '45%' },
  { height: 16, width: '75%' },
] as const;

export const extensionManagementTabIds = {
  tab: (index: number) => `extension-management-tab-${index}`,
  panel: (index: number) => `extension-management-tabpanel-${index}`,
};
export const extensionManagementTabIndex = {
  marketplace: 0,
  installed: 1,
  layout: 2,
} as const;

/** Host-owned outlet capacities; extension manifests never choose them. */
export const outletCapacities = {
  actionBarPrimary: 1,
  actionBarSecondary: 3,
  navigationPromoted: 3,
  visibleContent: 3,
} as const;
