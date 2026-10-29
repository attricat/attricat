import { useTranslation } from 'react-i18next';
import type { BlueprintWithAttributes, RecordItem } from '../records/api';
import { ExtensionPopoverOutlet } from '../extensions/ExtensionOutlet';
import {
  selectionSources,
  extensionOutlets,
  extensionPopoverTriggers,
  supportedOutletContextVersion,
} from '../extensions/constants';

type Blueprint = BlueprintWithAttributes['blueprint'];

const runtimeScope = (blueprint: Blueprint) => ({
  blueprintId: blueprint.id,
  blueprintVersion: blueprint.version,
});

const revisionContext = (blueprint: Blueprint) => ({
  context_version: supportedOutletContextVersion,
  blueprint_id: blueprint.id,
  blueprint_version: blueprint.version,
});

/**
 * Revision-scoped Explorer actions behind one toolbar button, which appears
 * once an extension renders any.
 */
export const ExplorerExtensionActions = ({
  blueprint,
}: {
  blueprint: Blueprint;
}) => {
  const { t } = useTranslation();
  return (
    <ExtensionPopoverOutlet
      context={revisionContext(blueprint)}
      label={t('explorer.extensionToolbarActions')}
      outlet={extensionOutlets.explorerAction}
      runtimeScope={runtimeScope(blueprint)}
      trigger={extensionPopoverTriggers.toolbar}
    />
  );
};

/**
 * Selection-scoped bulk actions as an item of the selection's actions menu,
 * for a selection from the shown revision.
 */
export const ExplorerBulkExtensionActions = ({
  blueprint,
  contextId,
  selectedItems,
}: {
  blueprint: Blueprint;
  /** The Explorer's value-resolution context, when one is selected. */
  contextId: string | undefined;
  selectedItems: RecordItem[];
}) => {
  const { t } = useTranslation();
  if (
    selectedItems.length === 0 ||
    selectedItems.some((item) => item.blueprint_version !== blueprint.version)
  )
    return null;
  const recordIds = selectedItems.map((item) => item.id);
  return (
    <ExtensionPopoverOutlet
      context={{ ...revisionContext(blueprint), record_ids: recordIds }}
      label={t('explorer.extensionActions')}
      outlet={extensionOutlets.explorerBulkAction}
      runtimeScope={runtimeScope(blueprint)}
      selection={{
        source: selectionSources.explorerSelection,
        blueprintId: blueprint.id,
        blueprintVersion: blueprint.version,
        contextId: contextId ?? null,
        recordIds,
      }}
      trigger={extensionPopoverTriggers.menuItem}
    />
  );
};
