import { Menu, MenuItem } from '@mui/material';
import { CopyIcon, SendIcon, Trash2Icon, Undo2Icon } from 'lucide-react';
import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { ActionMenuItem } from '../../components/ActionMenuItem';
import type {
  RecordItem,
  RecordPublicationReadiness,
  RecordPublicationStatus,
} from '../records/api';
import { accessTargetKinds, capabilities } from '../auth/capabilities';
import { useAccess } from '../auth/useCan';
import { publicationReadinessText } from '../records/checkViolations';
import { useActionDialogStore } from '../extensions/actionDialogStore';
import { ActionMenuCloseContext } from '../extensions/actions';
import { ExtensionActionMenuItems } from '../extensions/ExtensionActions';
import { useOfferedActions } from '../extensions/useOfferedActions';
import { useExplorerActionStates } from '../extensions/explorerFieldNotesContext';
import { extensionOutlets } from '../extensions/constants';
import { publicationStatuses } from '../records/constants';

type Props = {
  blueprintId: string;
  canPublish: boolean;
  canDelete: boolean;
  canDuplicate: boolean;
  record: RecordItem;
  onClose: () => void;
  onDelete: () => void;
  position: { left: number; top: number } | null;
  publication: RecordPublicationStatus | undefined;
  publicationContextId: string | undefined;
  publish: () => void;
  publishing: boolean;
  /** Check readiness of the publication channel; publishing waits for it. */
  readiness?: RecordPublicationReadiness;
  duplicate: () => void;
  unpublish: () => void;
  unpublishing: boolean;
};

export const RecordActionsMenu = ({
  blueprintId,
  canPublish,
  canDelete,
  canDuplicate,
  record,
  onClose,
  onDelete,
  position,
  publication,
  publicationContextId,
  publish,
  publishing,
  readiness,
  duplicate,
  unpublish,
  unpublishing,
}: Props) => {
  const { t } = useTranslation();
  const actionStates = useExplorerActionStates(record.id);
  const writeAccess = useAccess(
    capabilities.recordsWrite,
    accessTargetKinds.record,
    [record.id],
  );
  // A row action that opens the host dialog hands off to it; the dialog keeps
  // its captured selection after this menu and its frames unmount.
  const dialogKey = useActionDialogStore((state) => state.dialog?.key);
  const [initialDialogKey] = useState(dialogKey);
  useEffect(() => {
    if (dialogKey !== undefined && dialogKey !== initialDialogKey) onClose();
  }, [dialogKey, initialDialogKey, onClose]);
  const choose = (action: () => void) => () => {
    onClose();
    action();
  };
  const extensionActions = {
    runtimeScope: {
      blueprintId,
      blueprintVersion: record.blueprint_version,
    },
    states: actionStates,
    target: {
      blueprintId,
      blueprintVersion: record.blueprint_version,
      contextId: publicationContextId ?? null,
      recordIds: [record.id],
      recordTags: [record.system_tags],
      canWrite: writeAccess.isPending ? undefined : writeAccess.can(record.id),
    },
  };
  const offeredActions = useOfferedActions({
    ...extensionActions,
    outlet: extensionOutlets.explorerRowAction,
  });
  const hasActions =
    canDuplicate ||
    canDelete ||
    (canPublish && Boolean(publicationContextId && publication)) ||
    offeredActions.length > 0;

  return (
    <Menu
      anchorPosition={position ?? undefined}
      anchorReference="anchorPosition"
      onClose={onClose}
      open={Boolean(position)}
      transformOrigin={{ horizontal: 'right', vertical: 'top' }}
    >
      {!hasActions && (
        <MenuItem disabled>{t('explorer.noRecordActions')}</MenuItem>
      )}
      {canDuplicate && (
        <ActionMenuItem icon={CopyIcon} onClick={choose(duplicate)}>
          {t('records.duplicateRecord')}
        </ActionMenuItem>
      )}
      {canDelete && (
        <ActionMenuItem
          icon={Trash2Icon}
          onClick={choose(onDelete)}
          tone="destructive"
        >
          {t('records.deleteRecord')}
        </ActionMenuItem>
      )}
      {canPublish && publicationContextId && publication && (
        <>
          <ActionMenuItem
            disabled={publishing || readiness?.ready === false}
            icon={SendIcon}
            onClick={choose(publish)}
            secondary={
              readiness?.ready === false
                ? publicationReadinessText(readiness)
                : undefined
            }
          >
            {t('records.publish')}
          </ActionMenuItem>
          {publication.status !== publicationStatuses.notPublished && (
            <ActionMenuItem
              disabled={unpublishing}
              icon={Undo2Icon}
              onClick={choose(unpublish)}
              tone="reversal"
            >
              {t('records.unpublish')}
            </ActionMenuItem>
          )}
        </>
      )}
      <ActionMenuCloseContext value={onClose}>
        <ExtensionActionMenuItems
          {...extensionActions}
          outlet={extensionOutlets.explorerRowAction}
        />
      </ActionMenuCloseContext>
    </Menu>
  );
};
