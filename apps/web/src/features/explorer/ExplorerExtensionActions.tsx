import type { BlueprintWithAttributes, RecordItem } from '../records/api';
import { ExtensionOutlet } from '../extensions/ExtensionOutlet';
import {
  selectionSources,
  extensionOutlets,
  supportedOutletContextVersion,
} from '../extensions/constants';

type Props = {
  blueprint: BlueprintWithAttributes['blueprint'];
  /** The Explorer's value-resolution context, when one is selected. */
  contextId: string | undefined;
  selectedItems: RecordItem[] | null;
};

/**
 * Mounts revision-scoped Explorer actions and, for a selection from the same
 * revision, selection-scoped bulk actions.
 */
export const ExplorerExtensionActions = ({
  blueprint,
  contextId,
  selectedItems,
}: Props) => {
  const runtimeScope = {
    blueprintId: blueprint.id,
    blueprintVersion: blueprint.version,
  };
  const revisionContext = {
    context_version: supportedOutletContextVersion,
    blueprint_id: blueprint.id,
    blueprint_version: blueprint.version,
  };
  const bulkSelection =
    selectedItems &&
    selectedItems.length > 0 &&
    selectedItems.every((item) => item.blueprint_version === blueprint.version)
      ? selectedItems
      : null;

  return (
    <>
      <ExtensionOutlet
        context={revisionContext}
        outlet={extensionOutlets.explorerAction}
        runtimeScope={runtimeScope}
      />
      {bulkSelection && (
        <ExtensionOutlet
          context={{
            ...revisionContext,
            record_ids: bulkSelection.map((item) => item.id),
          }}
          outlet={extensionOutlets.explorerBulkAction}
          runtimeScope={runtimeScope}
          selection={{
            source: selectionSources.explorerSelection,
            blueprintId: blueprint.id,
            blueprintVersion: blueprint.version,
            contextId: contextId ?? null,
            recordIds: bulkSelection.map((item) => item.id),
          }}
        />
      )}
    </>
  );
};
