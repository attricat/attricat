import type { ExtensionContribution } from './api';
import { navigationOutlet, outletCapacities } from './constants';
import { outletContextSchemas } from './outletContextSchemas';

export type OutletName =
  | 'navigation'
  | 'entity_preview_panel'
  | 'blueprint_attribute_configuration'
  | 'entity_attribute_decoration'
  | 'entity_action'
  | 'entity_header_action'
  | 'explorer_row_action'
  | 'blueprint_detail_panel'
  | 'blueprint_panel'
  | 'audit_event_panel'
  | 'explorer_action'
  | 'explorer_bulk_action'
  | 'data_health_card'
  | 'blueprint_publish_check';

export type OutletContext = Record<string, unknown>;

export const contributionKey = (contribution: ExtensionContribution) =>
  `${contribution.extension_id}:${contribution.id}:${contribution.release_id}`;

const navigationPolicy = {
  kind: 'navigation',
  promotedCapacity: outletCapacities.navigationPromoted,
} as const;
const panelPolicy = {
  kind: 'panel',
  visibleCapacity: outletCapacities.visibleContent,
} as const;
const cardPolicy = {
  kind: 'card',
  visibleCapacity: outletCapacities.visibleContent,
} as const;
const popoverPolicy = { kind: 'popover' } as const;
const actionBarPolicy = {
  kind: 'actionBar',
  primaryCapacity: outletCapacities.actionBarPrimary,
  secondaryCapacity: outletCapacities.actionBarSecondary,
} as const;

export type ActionBarPolicy = typeof actionBarPolicy;
export type ContentPolicy = typeof panelPolicy | typeof cardPolicy;
export type NavigationPolicy = typeof navigationPolicy;

// Every mounted surface has an explicit host policy. Extension manifests never
// select capacity, grouping, or overflow behavior.
export const outletPolicies = {
  navigation: navigationPolicy,
  entity_preview_panel: panelPolicy,
  blueprint_attribute_configuration: panelPolicy,
  entity_attribute_decoration: popoverPolicy,
  entity_action: actionBarPolicy,
  entity_header_action: actionBarPolicy,
  explorer_row_action: popoverPolicy,
  blueprint_detail_panel: panelPolicy,
  blueprint_panel: panelPolicy,
  audit_event_panel: panelPolicy,
  explorer_action: actionBarPolicy,
  explorer_bulk_action: actionBarPolicy,
  data_health_card: cardPolicy,
  blueprint_publish_check: panelPolicy,
} as const satisfies Record<OutletName, { kind: string }>;

const embeddedOutlets = new Set<OutletName>([
  'navigation',
  'entity_preview_panel',
  'blueprint_attribute_configuration',
  'entity_attribute_decoration',
  'entity_action',
]);
const actionOutlets = new Set<OutletName>([
  'entity_header_action',
  'explorer_row_action',
  'explorer_action',
  'explorer_bulk_action',
]);
const panelOutlets = new Set<OutletName>([
  'blueprint_detail_panel',
  'blueprint_panel',
  'audit_event_panel',
  'data_health_card',
  'blueprint_publish_check',
]);

export const supportsOutlet = (
  contribution: ExtensionContribution,
  outlet: OutletName,
) =>
  contribution.outlet === outlet &&
  ((contribution.kind === 'embedded' && embeddedOutlets.has(outlet)) ||
    (contribution.kind === 'action' && actionOutlets.has(outlet)) ||
    (contribution.kind === 'panel' && panelOutlets.has(outlet)) ||
    (outlet === navigationOutlet &&
      contribution.kind === 'navigation' &&
      contribution.route !== null));

export const hasValidContext = (
  outlet: OutletName,
  context: OutletContext | undefined,
) => {
  const schema =
    outletContextSchemas[outlet as keyof typeof outletContextSchemas];
  return !schema || schema.safeParse(context).success;
};

/** Contributions mounted in an outlet, in host-computed display order. */
export const outletContributions = (
  runtime: ExtensionContribution[] | undefined,
  outlet: OutletName,
  context: OutletContext | undefined,
) =>
  runtime?.filter(
    (item) => supportsOutlet(item, outlet) && hasValidContext(outlet, context),
  ) ?? [];

/** Groups navigation contributions by extension, preserving display order. */
export const navigationByExtension = (
  contributions: ExtensionContribution[],
) => {
  const appsByExtension = new Map<string, ExtensionContribution[]>();
  contributions
    .filter((item) => item.kind === 'navigation')
    .forEach((item) => {
      const apps = appsByExtension.get(item.extension_id) ?? [];
      apps.push(item);
      appsByExtension.set(item.extension_id, apps);
    });
  return Array.from(appsByExtension.entries(), ([extensionId, apps]) => ({
    extensionId,
    extensionName: apps[0].extension_name,
    apps,
  }));
};
