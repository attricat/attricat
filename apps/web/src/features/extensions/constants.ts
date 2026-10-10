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
/** Selection-aware action contributions (contribution version 2) receive this context. */
export const selectionActionContributionVersion = 2;
export const selectionContextVersion = 2;
/** Mirrors the host's initial interactive selection bound. */
export const maximumActionSelection = 50;

/** Host-owned insertion points an extension contribution may target. */
export const extensionOutletNames = [
  'navigation',
  'record_preview_panel',
  'blueprint_attribute_configuration',
  'record_attribute_decoration',
  'record_action',
  'explorer_row_action',
  'explorer_table_cell',
  'blueprint_detail_panel',
  'explorer_action',
  'explorer_bulk_action',
  'record_header_action',
  'record_attribute_panel',
  'blueprint_panel',
  'blueprint_publish_check',
  'file_panel',
  'audit_event_panel',
  'data_health_card',
  'action_dialog',
] as const;
export const navigationOutlet = 'navigation';
/** The host-managed dialog opened by an extension's selection actions. */
export const actionDialogOutlet = 'action_dialog';
/** Outlets whose version 2 contributions receive a normalized selection. */
export const selectionActionOutlets = [
  'record_action',
  'explorer_row_action',
  'explorer_bulk_action',
] as const;
export const selectionSources = {
  recordPreview: 'record_preview',
  explorerRow: 'explorer_row',
  explorerSelection: 'explorer_selection',
} as const;

export const contributionKinds = [
  'route',
  'navigation',
  'embedded',
  'action',
  'panel',
  'dialog',
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
  actionDialog: 'client.action_dialog',
  attricatRead: 'attricat.read',
  operationsCancel: 'client.operations.cancel',
  operationsRead: 'client.operations.read',
  operationsStart: 'client.operations.start',
  commands: 'client.commands',
  events: 'client.events',
  navigation: 'client.navigation',
  notification: 'client.notification',
  refresh: 'client.refresh',
  storage: 'storage.extension',
} as const;

/** Versioned host/frame MessageChannel protocol message types. */
export const extensionMessageTypes = {
  contextChanged: 'attricat:context-changed.v1',
  contextUpdate: 'attricat:context-update.v1',
  error: 'attricat:error.v1',
  init: 'attricat:init.v1',
  ready: 'attricat:ready.v1',
  request: 'attricat:request.v1',
  resize: 'attricat:resize.v1',
  response: 'attricat:response.v1',
  shutdown: 'attricat:shutdown.v1',
  themeChanged: 'attricat:theme-changed.v1',
  themeUpdate: 'attricat:theme-update.v1',
} as const;

/** Broker methods an extension frame can call through its MessagePort. */
export const extensionBrokerMethods = {
  attricatRead: 'attricat.read',
  command: 'command',
  dialogClose: 'dialog.close',
  dialogOpen: 'dialog.open',
  operationsCancel: 'operations.cancel',
  operationsDownload: 'operations.download',
  operationsGet: 'operations.get',
  operationsList: 'operations.list',
  operationsStart: 'operations.start',
  navigate: 'navigate',
  notify: 'notify',
  refresh: 'refresh',
} as const;
export const extensionStorageMethodPrefix = 'storage.';
export const operationMethodPrefix = 'operations.';
/** Mirrors the host's client idempotency-key bound for interactive runs. */
export const maximumOperationIdempotencyKeyLength = 64;
/** Visible ASCII only, matching the host's idempotency-key validation. */
export const operationIdempotencyKeyPattern = /^[\x21-\x7e]+$/;
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
  missingMountExport: 'Extension must export mount(root, attricat)',
  attricatResponseTooLarge: 'Attricat response is too large',
  commandPayloadTooLarge: 'Command payload is too large',
  invalidStorageKey: 'Invalid storage key',
  operationInputTooLarge: 'Operation input is too large',
  selectionUnavailable: 'This contribution has no selection context',
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

// Components may read current record data or the exact blueprint revision named
// by the blueprint-configuration outlet. They cannot supply methods, query
// strings, arbitrary URLs, or credentials.
export const attricatReadPathPattern =
  /^\/api\/(?:records|v1\/records\/[0-9a-f-]{36}|blueprints\/[0-9a-f-]{36}\/versions\/[1-9][0-9]*)$/;
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
