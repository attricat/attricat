import type { ExtensionContribution } from './api';
import {
  contributionKinds,
  extensionCapabilities,
  extensionOutlets,
  supportedOutletContextVersion,
} from './constants';

/**
 * An extension's tab on a record page, keyed like its layout entry. Built-in
 * tab keys never contain the separator.
 */
export type ExtensionRecordTabKey = `${string}:${string}`;

export const recordTabKey = (
  extensionId: string,
  contributionId: string,
): ExtensionRecordTabKey => `${extensionId}:${contributionId}`;

export const isExtensionRecordTabKey = (
  key: string,
): key is ExtensionRecordTabKey => key.includes(':');

/** The only page data a record tab's frame receives. */
export type RecordTabContext = {
  context_version: typeof supportedOutletContextVersion;
  record_id: string;
  context_id?: string;
  blueprint_id: string;
  blueprint_version: number;
};

/**
 * Tabs extensions add to a record of `blueprintCode`, in host-computed
 * display order. A tab shows only once its capability is granted.
 */
export const recordTabContributions = (
  runtime: ExtensionContribution[] | undefined,
  blueprintCode: string | undefined,
) =>
  runtime?.filter(
    (item) =>
      item.outlet === extensionOutlets.recordTab &&
      item.kind === contributionKinds.tab &&
      item.capabilities.includes(extensionCapabilities.recordTab) &&
      blueprintCode !== undefined &&
      (!item.when?.blueprints?.length ||
        item.when.blueprints.includes(blueprintCode)),
  ) ?? [];
