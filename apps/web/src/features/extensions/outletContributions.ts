import { z } from 'zod';
import {
  isSelectionContribution,
  selectionContext,
  selectionContextSchema,
  type ActionSelection,
} from './actionSelection';
import type { ExtensionContribution } from './api';
import { navigationOutlet, outletCapacities } from './constants';
import { outletContextSchemas } from './outletContextSchemas';
import { extensionOutletSchema } from './schemas';

// The Explorer table-cell renderer and the action dialog have their own
// host-controlled mount paths.
export type OutletName = Exclude<
  z.infer<typeof extensionOutletSchema>,
  'explorer_table_cell' | 'action_dialog'
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
  record_preview_panel: panelPolicy,
  blueprint_attribute_configuration: panelPolicy,
  record_attribute_decoration: popoverPolicy,
  record_action: actionBarPolicy,
  record_header_action: actionBarPolicy,
  explorer_row_action: popoverPolicy,
  blueprint_detail_panel: panelPolicy,
  blueprint_panel: panelPolicy,
  audit_event_panel: panelPolicy,
  explorer_action: actionBarPolicy,
  explorer_bulk_action: actionBarPolicy,
  data_health_card: cardPolicy,
  blueprint_publish_check: panelPolicy,
  record_attribute_panel: panelPolicy,
  file_panel: panelPolicy,
} as const satisfies Record<OutletName, { kind: string }>;

const embeddedOutlets = new Set<OutletName>([
  'navigation',
  'record_preview_panel',
  'blueprint_attribute_configuration',
  'record_attribute_decoration',
  'record_action',
]);
const actionOutlets = new Set<OutletName>([
  'record_header_action',
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
  'record_attribute_panel',
  'file_panel',
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

/**
 * The context one contribution receives. Selection-aware (version 2) action
 * contributions get the normalized selection; all others keep their released
 * outlet context, so old releases are unaffected.
 */
export const contributionContext = (
  contribution: ExtensionContribution,
  context: OutletContext | undefined,
  selection: ActionSelection | null | undefined,
): OutletContext | undefined =>
  isSelectionContribution(contribution)
    ? selection
      ? selectionContext(selection)
      : undefined
    : context;

/** Contributions mounted in an outlet, in host-computed display order. */
export const outletContributions = (
  runtime: ExtensionContribution[] | undefined,
  outlet: OutletName,
  context: OutletContext | undefined,
  selection?: ActionSelection | null,
) =>
  runtime?.filter(
    (item) =>
      supportsOutlet(item, outlet) &&
      (isSelectionContribution(item)
        ? selectionContextSchema.safeParse(
            contributionContext(item, context, selection),
          ).success
        : hasValidContext(outlet, context)),
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
