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
import { Button, Divider, Menu } from '@mui/material';
import { useId, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { RouterMenuItem } from '../../../components/RouterLink';
import { useCopyToClipboard } from '../../../components/useCopyToClipboard';
import {
  ActionMenuItem,
  ActionMenuItemContent,
} from '../../../components/ActionMenuItem';
import { lexiconText } from '../../lexicon/lexicon';
import { RecordIdIcon, AuditLogIcon } from '../../../components/systemIcons';
import { publicationStatuses } from '../constants';
import { publicationReadinessText } from '../checkViolations';
import type { RecordPublications } from './useRecordPublications';
import { ActionMenuCloseContext } from '../../extensions/actions';
import { useActionMenuAnchor } from '../../extensions/useActionMenuAnchor';

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
  const { anchor, close, open } = useActionMenuAnchor();
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
        onClick={open}
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
          <ActionMenuItemContent icon={AuditLogIcon}>
            {t('records.changes')}
          </ActionMenuItemContent>
        </RouterMenuItem>
        <ActionMenuItem
          icon={RecordIdIcon}
          onClick={choose(() => void copyText(recordId))}
        >
          {t('records.copyRecordId')}
        </ActionMenuItem>
        {/* Duplicating creates a record, which needs a workspace grant. */}
        {canCreate && (
          <ActionMenuItem icon={CopyIcon} onClick={choose(onDuplicate)}>
            {t('records.duplicateRecord')}
          </ActionMenuItem>
        )}
        {canCreate && blueprint && (
          <RouterMenuItem
            onClick={close}
            search={{ blueprint: blueprint.code }}
            to="/records/new"
          >
            <ActionMenuItemContent icon={PlusIcon}>
              {t('records.createAnother', {
                blueprint: lexiconText(blueprint.name),
              })}
            </ActionMenuItemContent>
          </RouterMenuItem>
        )}
        {canWrite && schemaOutdated && (
          <RouterMenuItem
            onClick={close}
            params={{ recordId }}
            to="/records/$recordId/migrate"
          >
            <ActionMenuItemContent icon={CircleArrowUpIcon}>
              {t('records.upgradeBlueprint')}
            </ActionMenuItemContent>
          </RouterMenuItem>
        )}
        <ActionMenuCloseContext value={close}>
          {extensionActions}
        </ActionMenuCloseContext>
        {publications && <Divider />}
        {publications && publication && (
          <ActionMenuItem
            disabled={publications.isPending || notReady}
            icon={
              publication.status === publicationStatuses.notPublished
                ? SendIcon
                : RefreshCwIcon
            }
            onClick={choose(publications.publish)}
            secondary={notReady ? readinessText : undefined}
          >
            {publication.status === publicationStatuses.notPublished
              ? t('records.publish')
              : t('records.republish')}
          </ActionMenuItem>
        )}
        {publications && (
          <ActionMenuItem
            disabled={publications.isPending || notReadyChannels.length > 0}
            icon={GlobeIcon}
            onClick={choose(publications.publishAll)}
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
          </ActionMenuItem>
        )}
        {publications &&
          publication?.status === publicationStatuses.published && (
            <ActionMenuItem
              disabled={publications.isPending}
              icon={Undo2Icon}
              onClick={choose(publications.unpublish)}
              tone="reversal"
            >
              {t('records.unpublish')}
            </ActionMenuItem>
          )}
        {canDelete && <Divider />}
        {canDelete && (
          <ActionMenuItem
            icon={Trash2Icon}
            onClick={choose(onDelete)}
            tone="destructive"
          >
            {t('records.deleteRecord')}
          </ActionMenuItem>
        )}
      </Menu>
    </>
  );
};
