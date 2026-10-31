import { ListItemIcon, ListItemText, Menu, MenuItem } from '@mui/material';
import {
  CopyIcon,
  InfoIcon,
  SendIcon,
  Trash2Icon,
  Undo2Icon,
} from 'lucide-react';
import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { compactIconSize } from '../../components/iconSizes';
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
  onSearchInfo: (record: RecordItem) => void;
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
  onSearchInfo,
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

  return (
    <Menu
      anchorPosition={position ?? undefined}
      anchorReference="anchorPosition"
      onClose={onClose}
      open={Boolean(position)}
      transformOrigin={{ horizontal: 'right', vertical: 'top' }}
    >
      <MenuItem
        onClick={() => {
          onClose();
          onSearchInfo(record);
        }}
      >
        <ListItemIcon>
          <InfoIcon size={compactIconSize} />
        </ListItemIcon>
        <ListItemText>{t('explorer.searchInfo')}</ListItemText>
      </MenuItem>
      {canDuplicate && (
        <MenuItem
          onClick={() => {
            onClose();
            duplicate();
          }}
        >
          <ListItemIcon>
            <CopyIcon size={compactIconSize} />
          </ListItemIcon>
          <ListItemText>{t('records.duplicateRecord')}</ListItemText>
        </MenuItem>
      )}
      {canDelete && (
        <MenuItem
          onClick={() => {
            onClose();
            onDelete();
          }}
          sx={{ color: 'error.main' }}
        >
          <ListItemIcon sx={{ color: 'inherit' }}>
            <Trash2Icon size={compactIconSize} />
          </ListItemIcon>
          <ListItemText>{t('records.deleteRecord')}</ListItemText>
        </MenuItem>
      )}
      {canPublish && publicationContextId && publication && (
        <>
          <MenuItem
            disabled={publishing || readiness?.ready === false}
            onClick={() => {
              onClose();
              publish();
            }}
          >
            <ListItemIcon>
              <SendIcon size={compactIconSize} />
            </ListItemIcon>
            <ListItemText
              primary={t('records.publish')}
              secondary={
                readiness?.ready === false
                  ? publicationReadinessText(readiness)
                  : undefined
              }
            />
          </MenuItem>
          {publication.status !== publicationStatuses.notPublished && (
            <MenuItem
              disabled={unpublishing}
              onClick={() => {
                onClose();
                unpublish();
              }}
              sx={{ color: 'warning.main' }}
            >
              <ListItemIcon sx={{ color: 'inherit' }}>
                <Undo2Icon size={compactIconSize} />
              </ListItemIcon>
              <ListItemText>{t('records.unpublish')}</ListItemText>
            </MenuItem>
          )}
        </>
      )}
      <ActionMenuCloseContext value={onClose}>
        <ExtensionActionMenuItems
          outlet={extensionOutlets.explorerRowAction}
          runtimeScope={{
            blueprintId,
            blueprintVersion: record.blueprint_version,
          }}
          states={actionStates}
          target={{
            blueprintId,
            blueprintVersion: record.blueprint_version,
            contextId: publicationContextId ?? null,
            recordIds: [record.id],
            recordTags: [record.system_tags],
            canWrite: writeAccess.isPending
              ? undefined
              : writeAccess.can(record.id),
          }}
        />
      </ActionMenuCloseContext>
    </Menu>
  );
};
