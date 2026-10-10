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
import { publicationReadinessText } from '../records/checkViolations';
import { useActionDialogStore } from '../extensions/actionDialogStore';
import { ExtensionPopoverOutlet } from '../extensions/ExtensionOutlet';
import { selectionSources } from '../extensions/constants';
import {
  explorerExtensionContextVersion,
  explorerExtensionOutlets,
  publicationStatuses,
} from './constants';

type Props = {
  blueprintId: string;
  canPublish: boolean;
  canDelete: boolean;
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
      <ExtensionPopoverOutlet
        context={{
          context_version: explorerExtensionContextVersion,
          blueprint_id: blueprintId,
          blueprint_version: record.blueprint_version,
          record_id: record.id,
        }}
        label={t('explorer.extensionActions')}
        outlet={explorerExtensionOutlets.rowAction}
        selection={{
          source: selectionSources.explorerRow,
          blueprintId,
          blueprintVersion: record.blueprint_version,
          contextId: publicationContextId ?? null,
          recordIds: [record.id],
        }}
      />
    </Menu>
  );
};
