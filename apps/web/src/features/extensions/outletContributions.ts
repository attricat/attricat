import { z } from 'zod';
import type { ExtensionContribution } from './api';
import {
  outletCapacities,
  contributionKinds,
  extensionFramePlaceholders,
  extensionOutlets,
  type ActionOutlet,
} from './constants';
import { outletContextSchemas } from './outletContextSchemas';
import { extensionOutletSchema } from './schemas';

// Outlets that mount extension frames. The Explorer table-cell renderer and
// the action dialog have their own host-controlled mount paths, and action
// outlets render host-owned items (see ExtensionActions).
export type OutletName = Exclude<
  z.infer<typeof extensionOutletSchema>,
  | typeof extensionOutlets.explorerTableCell
  | typeof extensionOutlets.actionDialog
  | ActionOutlet
>;

export type OutletContext = Record<string, unknown>;

export const contributionKey = (contribution: ExtensionContribution) =>
  `${contribution.extension_id}:${contribution.id}:${contribution.release_id}`;

const navigationPolicy = {
  kind: 'navigation',
  promotedCapacity: outletCapacities.navigationPromoted,
} as const;
const panelPolicy = {
  kind: 'panel',
  placeholder: extensionFramePlaceholders.skeleton,
  visibleCapacity: outletCapacities.visibleContent,
} as const;
// Repeated once per item beside other content: it takes no space until an
// extension renders something, so a page of empty slots never moves.
const inlinePanelPolicy = {
  ...panelPolicy,
  placeholder: extensionFramePlaceholders.none,
} as const;
const cardPolicy = {
  kind: 'card',
  visibleCapacity: outletCapacities.visibleContent,
} as const;

export type PanelPolicy = typeof panelPolicy | typeof inlinePanelPolicy;
export type ContentPolicy = PanelPolicy | typeof cardPolicy;
export type NavigationPolicy = typeof navigationPolicy;

// Every mounted surface has an explicit host policy. Extension manifests never
// select capacity, grouping, or overflow behavior.
export const outletPolicies = {
  navigation: navigationPolicy,
  record_preview_panel: panelPolicy,
  blueprint_attribute_configuration: inlinePanelPolicy,
  blueprint_detail_panel: panelPolicy,
  blueprint_panel: panelPolicy,
  audit_event_panel: panelPolicy,
  data_health_card: cardPolicy,
  blueprint_publish_check: panelPolicy,
  export_mapping: panelPolicy,
} as const satisfies Record<OutletName, { kind: string }>;

const embeddedOutlets = new Set<OutletName>([
  extensionOutlets.navigation,
  extensionOutlets.recordPreviewPanel,
  extensionOutlets.blueprintAttributeConfiguration,
]);
const panelOutlets = new Set<OutletName>([
  extensionOutlets.blueprintDetailPanel,
  extensionOutlets.blueprintPanel,
  extensionOutlets.auditEventPanel,
  extensionOutlets.dataHealthCard,
  extensionOutlets.blueprintPublishCheck,
  extensionOutlets.exportMapping,
]);

const supportsOutlet = (
  contribution: ExtensionContribution,
  outlet: OutletName,
) =>
  contribution.outlet === outlet &&
  ((contribution.kind === contributionKinds.embedded &&
    embeddedOutlets.has(outlet)) ||
    (contribution.kind === contributionKinds.panel &&
      panelOutlets.has(outlet)) ||
    (outlet === extensionOutlets.navigation &&
      contribution.kind === contributionKinds.navigation &&
      contribution.route !== null));

const hasValidContext = (
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
    .filter((item) => item.kind === contributionKinds.navigation)
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
