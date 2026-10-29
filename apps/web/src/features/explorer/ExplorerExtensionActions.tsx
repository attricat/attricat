import { useTranslation } from 'react-i18next';
import type { BlueprintWithAttributes, RecordItem } from '../records/api';
import {
  ExtensionActionMenuItems,
  ExtensionActionsMenuButton,
} from '../extensions/ExtensionActions';
import { extensionOutlets } from '../extensions/constants';

type Blueprint = BlueprintWithAttributes['blueprint'];

const runtimeScope = (blueprint: Blueprint) => ({
  blueprintId: blueprint.id,
  blueprintVersion: blueprint.version,
});

/**
 * Revision-scoped Explorer actions behind one toolbar button, which appears
 * once an extension contributes any. They name no rows.
 */
export const ExplorerExtensionActions = ({
  blueprint,
  contextId,
}: {
  blueprint: Blueprint;
  /** The Explorer's value-resolution context, when one is selected. */
  contextId: string | undefined;
}) => {
  const { t } = useTranslation();
  return (
    <ExtensionActionsMenuButton
      label={t('explorer.extensionToolbarActions')}
      outlet={extensionOutlets.explorerAction}
      runtimeScope={runtimeScope(blueprint)}
      target={{
        blueprintId: blueprint.id,
        blueprintVersion: blueprint.version,
        contextId: contextId ?? null,
        recordIds: [],
      }}
    />
  );
};

/**
 * Selection-scoped bulk actions as items of the selection's actions menu,
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
  if (
    selectedItems.length === 0 ||
    selectedItems.some((item) => item.blueprint_version !== blueprint.version)
  )
    return null;
  return (
    <ExtensionActionMenuItems
      outlet={extensionOutlets.explorerBulkAction}
      runtimeScope={runtimeScope(blueprint)}
      target={{
        blueprintId: blueprint.id,
        blueprintVersion: blueprint.version,
        contextId: contextId ?? null,
        recordIds: selectedItems.map((item) => item.id),
      }}
    />
  );
};
