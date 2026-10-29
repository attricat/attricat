import {
  ChevronDownIcon,
  CircleArrowUpIcon,
  CopyIcon,
  GlobeIcon,
  PlusIcon,
  RefreshCwIcon,
  SendIcon,
  Trash2Icon,
  Undo2Icon,
} from 'lucide-react';
import {
  Button,
  Divider,
  ListItemIcon,
  ListItemText,
  Menu,
  MenuItem,
} from '@mui/material';
import { useId, useState, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { RouterMenuItem } from '../../../components/RouterLink';
import { useCopyToClipboard } from '../../../components/useCopyToClipboard';
import { compactIconSize } from '../../../components/iconSizes';
import { lexiconText } from '../../lexicon/lexicon';
import { RecordIdIcon, AuditLogIcon } from '../../../components/systemIcons';
import { publicationStatuses } from '../constants';
import { publicationReadinessText } from '../checkViolations';
import type { RecordPublications } from './useRecordPublications';
import { useActionDialogStore } from '../../extensions/actionDialogStore';
import { ActionMenuCloseContext } from '../../extensions/actions';

/**
 * The record's Actions menu: history, copying, creating, extension actions,
 * publishing and deleting.
 */
export const RecordActionsMenu = ({
  blueprint,
  canCreate,
  canDelete,
  canWrite,
  extensionActions,
  onDelete,
  onDuplicate,
  publications,
  recordId,
  schemaOutdated,
}: {
  /** The record's blueprint, offered for creating another record. */
  blueprint?: { code: string; name: string };
  /** The caller may create records in the workspace. */
  canCreate: boolean;
  canDelete: boolean;
  /** The caller may change this record (duplicate it or upgrade its blueprint). */
  canWrite: boolean;
  onDelete: () => void;
  onDuplicate: () => void;
  /** Extension header actions, rendered as menu items. */
  extensionActions?: ReactNode;
  /** Publishes or withdraws the record; given when the caller may publish. */
  publications?: RecordPublications;
  recordId: string;
  schemaOutdated?: boolean;
}) => {
  const { t } = useTranslation();
  const menuId = useId();
  const [anchor, setAnchor] = useState<HTMLElement | null>(null);
  const close = () => setAnchor(null);
  // An extension action that opens the host dialog hands off to it.
  const dialogKey = useActionDialogStore((state) => state.dialog?.key);
  const [openedDialogKey, setOpenedDialogKey] = useState(dialogKey);
  if (anchor && dialogKey !== undefined && dialogKey !== openedDialogKey) {
    setOpenedDialogKey(dialogKey);
    setAnchor(null);
  }
  const copyText = useCopyToClipboard();
  const choose = (action: () => void) => () => {
    close();
    action();
  };
  const publication = publications?.publication;
  const notReady = publications?.readiness?.ready === false;
  const readinessText =
    publications?.readiness && publicationReadinessText(publications.readiness);
  const notReadyChannels = publications?.notReadyChannels ?? [];
  return (
    <>
      <Button
        aria-controls={anchor ? menuId : undefined}
        aria-expanded={Boolean(anchor)}
        aria-haspopup="menu"
        color="inherit"
        endIcon={<ChevronDownIcon />}
        onClick={(event) => {
          setOpenedDialogKey(dialogKey);
          setAnchor(event.currentTarget);
        }}
        size="small"
      >
        {t('records.recordActions')}
      </Button>
      <Menu
        anchorEl={anchor}
        id={menuId}
        // Extension actions load with the page, so their item is ready when
        // the menu opens.
        keepMounted
        onClose={close}
        open={Boolean(anchor)}
      >
        <RouterMenuItem
          onClick={close}
          params={{ recordId }}
          to="/records/$recordId/changes"
        >
          <ListItemIcon>
            <AuditLogIcon size={compactIconSize} />
          </ListItemIcon>
          <ListItemText>{t('records.changes')}</ListItemText>
        </RouterMenuItem>
        <MenuItem onClick={choose(() => void copyText(recordId))}>
          <ListItemIcon>
            <RecordIdIcon size={compactIconSize} />
          </ListItemIcon>
          <ListItemText>{t('records.copyRecordId')}</ListItemText>
        </MenuItem>
        {canWrite && (
          <MenuItem onClick={choose(onDuplicate)}>
            <ListItemIcon>
              <CopyIcon size={compactIconSize} />
            </ListItemIcon>
            <ListItemText>{t('records.duplicateRecord')}</ListItemText>
          </MenuItem>
        )}
        {canCreate && blueprint && (
          <RouterMenuItem
            onClick={close}
            search={{ blueprint: blueprint.code }}
            to="/records/new"
          >
            <ListItemIcon>
              <PlusIcon size={compactIconSize} />
            </ListItemIcon>
            <ListItemText>
              {t('records.createAnother', {
                blueprint: lexiconText(blueprint.name),
              })}
            </ListItemText>
          </RouterMenuItem>
        )}
        {canWrite && schemaOutdated && (
          <RouterMenuItem
            onClick={close}
            params={{ recordId }}
            to="/records/$recordId/migrate"
          >
            <ListItemIcon>
              <CircleArrowUpIcon size={compactIconSize} />
            </ListItemIcon>
            <ListItemText>{t('records.upgradeBlueprint')}</ListItemText>
          </RouterMenuItem>
        )}
        <ActionMenuCloseContext value={close}>
          {extensionActions}
        </ActionMenuCloseContext>
        {publications && <Divider />}
        {publications && publication && (
          <MenuItem
            disabled={publications.isPending || notReady}
            onClick={choose(publications.publish)}
          >
            <ListItemIcon>
              {publication.status === publicationStatuses.notPublished ? (
                <SendIcon size={compactIconSize} />
              ) : (
                <RefreshCwIcon size={compactIconSize} />
              )}
            </ListItemIcon>
            <ListItemText secondary={notReady ? readinessText : undefined}>
              {publication.status === publicationStatuses.notPublished
                ? t('records.publish')
                : t('records.republish')}
            </ListItemText>
          </MenuItem>
        )}
        {publications && (
          <MenuItem
            disabled={publications.isPending || notReadyChannels.length > 0}
            onClick={choose(publications.publishAll)}
          >
            <ListItemIcon>
              <GlobeIcon size={compactIconSize} />
            </ListItemIcon>
            <ListItemText
              secondary={
                notReadyChannels.length > 0
                  ? t('records.publication.channelsNotReady', {
                      channels: notReadyChannels
                        .map((channel) => channel.context_code)
                        .join(', '),
                    })
                  : undefined
              }
            >
              {t('records.publishAllChannels')}
            </ListItemText>
          </MenuItem>
        )}
        {publications &&
          publication?.status === publicationStatuses.published && (
            <MenuItem
              disabled={publications.isPending}
              onClick={choose(publications.unpublish)}
              sx={{ color: 'warning.main' }}
            >
              <ListItemIcon sx={{ color: 'inherit' }}>
                <Undo2Icon size={compactIconSize} />
              </ListItemIcon>
              <ListItemText>{t('records.unpublish')}</ListItemText>
            </MenuItem>
          )}
        {canDelete && <Divider />}
        {canDelete && (
          <MenuItem onClick={choose(onDelete)} sx={{ color: 'error.main' }}>
            <ListItemIcon sx={{ color: 'inherit' }}>
              <Trash2Icon size={compactIconSize} />
            </ListItemIcon>
            <ListItemText>{t('records.deleteRecord')}</ListItemText>
          </MenuItem>
        )}
      </Menu>
    </>
  );
};
