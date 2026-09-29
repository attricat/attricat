import type { BlueprintWithAttributes, EntityItem } from '../entities/api';
import { ExtensionOutlet } from '../extensions/ExtensionOutlet';
import {
  explorerExtensionContextVersion,
  explorerExtensionOutlets,
} from './constants';

type Props = {
  blueprint: BlueprintWithAttributes['blueprint'];
  selectedItems: EntityItem[] | null;
};

/**
 * Mounts revision-scoped Explorer actions and, for a selection from the same
 * revision, selection-scoped bulk actions.
 */
export const ExplorerExtensionActions = ({
  blueprint,
  selectedItems,
}: Props) => {
  const runtimeScope = {
    blueprintId: blueprint.id,
    blueprintVersion: blueprint.version,
  };
  const revisionContext = {
    context_version: explorerExtensionContextVersion,
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
        outlet={explorerExtensionOutlets.action}
        runtimeScope={runtimeScope}
      />
      {bulkSelection && (
        <ExtensionOutlet
          context={{
            ...revisionContext,
            entity_ids: bulkSelection.map((item) => item.id),
          }}
          outlet={explorerExtensionOutlets.bulkAction}
          runtimeScope={runtimeScope}
        />
      )}
    </>
  );
};
