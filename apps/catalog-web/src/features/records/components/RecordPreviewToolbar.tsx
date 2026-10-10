import {
  ChevronDownIcon,
  CircleArrowUpIcon,
  CopyIcon,
  GlobeIcon,
  HashIcon,
  InfoIcon,
  PanelRightIcon,
  RefreshCwIcon,
  RotateCcwClockIcon,
  SendIcon,
  Trash2Icon,
  Undo2Icon,
} from 'lucide-react';
import { AgentIcon } from '../../../components/systemIcons';
import {
  Box,
  Button,
  Chip,
  Divider,
  IconButton,
  ListItemIcon,
  ListItemText,
  Menu,
  MenuItem,
  Tooltip,
} from '@mui/material';
import { useId, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { RouterMenuItem } from '../../../components/RouterLink';
import { copyToClipboard } from '../../../components/clipboard';
import { useToast } from '../../../components/useToast';
import { RecordToolbar } from './RecordToolbar';
import type {
  RecordPublicationReadiness,
  RecordPublicationStatus,
} from '../schemas';
import { RECORD_EXTENSION_DRAWER_ID, publicationStatuses } from '../constants';
import { publicationReadinessText } from '../checkViolations';
import { compactIconSize } from '../../../components/iconSizes';
import { useInstantFormat } from '../../../time/useInstantFormat';

type Props = {
  /** Moves the agent and extension buttons into the actions menu. */
  compact?: boolean;
  recordId: string;
  extensionPanelOpen: boolean;
  onOpenExtensions: () => void;
  agentPanelOpen: boolean;
  onOpenAgent: () => void;
  schemaOutdated?: boolean;
  showExtensions: boolean;
  onDuplicate: () => void;
  canDelete: boolean;
  onDelete: () => void;
  publication?: RecordPublicationStatus;
  canPublish: boolean;
  /** Check readiness of the selected context's channel. */
  readiness?: RecordPublicationReadiness;
  /** Enabled channels whose checks currently fail. */
  notReadyChannels?: readonly RecordPublicationReadiness[];
  onPublish: () => void;
  onPublishAll: () => void;
  onUnpublish: () => void;
  publicationPending: boolean;
};

export const RecordPreviewToolbar = ({
  compact = false,
  recordId,
  extensionPanelOpen,
  onOpenExtensions,
  agentPanelOpen,
  onOpenAgent,
  schemaOutdated,
  showExtensions,
  onDuplicate,
  canDelete,
  onDelete,
  publication,
  canPublish,
  readiness,
  notReadyChannels = [],
  onPublish,
  onPublishAll,
  onUnpublish,
  publicationPending,
}: Props) => {
  const { t } = useTranslation();
  const { show } = useToast();
  // A tooltip cannot host another tooltip, so the UTC value is inline.
  const { formatWithUtc } = useInstantFormat();
  const actionsMenuId = useId();
  const [actionsMenuAnchor, setActionsMenuAnchor] =
    useState<HTMLElement | null>(null);
  const closeActionsMenu = () => setActionsMenuAnchor(null);
  const copyRecordId = async () => {
    try {
      await copyToClipboard(recordId);
      show({ message: t('common.copied'), severity: 'success' });
    } catch {
      show({ message: t('common.copyFailed'), severity: 'error' });
    }
  };
  const publishMenuId = useId();
  const [publishMenuAnchor, setPublishMenuAnchor] =
    useState<HTMLElement | null>(null);
  const closePublishMenu = () => setPublishMenuAnchor(null);
  const notReady = readiness?.ready === false;
  const readinessText = readiness && publicationReadinessText(readiness);
  return (
    <RecordToolbar label={t('records.recordPreview')}>
      <Button
        aria-controls={actionsMenuAnchor ? actionsMenuId : undefined}
        aria-expanded={Boolean(actionsMenuAnchor)}
        aria-haspopup="menu"
        color="inherit"
        endIcon={<ChevronDownIcon />}
        onClick={(event) => setActionsMenuAnchor(event.currentTarget)}
        size="small"
      >
        {t('records.recordActions')}
      </Button>
      <Menu
        anchorEl={actionsMenuAnchor}
        id={actionsMenuId}
        onClose={closeActionsMenu}
        open={Boolean(actionsMenuAnchor)}
      >
        <RouterMenuItem
          onClick={closeActionsMenu}
          params={{ recordId }}
          to="/records/$recordId/changes"
        >
          <ListItemIcon>
            <RotateCcwClockIcon size={compactIconSize} />
          </ListItemIcon>
          <ListItemText>{t('records.changes')}</ListItemText>
        </RouterMenuItem>
        <MenuItem
          onClick={() => {
            closeActionsMenu();
            void copyRecordId();
          }}
        >
          <ListItemIcon>
            <HashIcon size={compactIconSize} />
          </ListItemIcon>
          <ListItemText>{t('records.copyRecordId')}</ListItemText>
        </MenuItem>
        <MenuItem
          onClick={() => {
            closeActionsMenu();
            onDuplicate();
          }}
        >
          <ListItemIcon>
            <CopyIcon size={compactIconSize} />
          </ListItemIcon>
          <ListItemText>{t('records.duplicateRecord')}</ListItemText>
        </MenuItem>
        {schemaOutdated && (
          <RouterMenuItem
            onClick={closeActionsMenu}
            params={{ recordId }}
            to="/records/$recordId/migrate"
          >
            <ListItemIcon>
              <CircleArrowUpIcon size={compactIconSize} />
            </ListItemIcon>
            <ListItemText>{t('records.upgradeBlueprint')}</ListItemText>
          </RouterMenuItem>
        )}
        {compact && showExtensions && <Divider />}
        {compact && showExtensions && (
          <MenuItem
            onClick={() => {
              closeActionsMenu();
              onOpenAgent();
            }}
          >
            <ListItemIcon>
              <AgentIcon size={compactIconSize} />
            </ListItemIcon>
            <ListItemText>{t('records.askAboutRecord')}</ListItemText>
          </MenuItem>
        )}
        {compact && showExtensions && (
          <MenuItem
            aria-controls={RECORD_EXTENSION_DRAWER_ID}
            onClick={() => {
              closeActionsMenu();
              onOpenExtensions();
            }}
          >
            <ListItemIcon>
              <PanelRightIcon size={compactIconSize} />
            </ListItemIcon>
            <ListItemText>{t('records.extensionContributions')}</ListItemText>
          </MenuItem>
        )}
        {canDelete && <Divider />}
        {canDelete && (
          <MenuItem
            onClick={() => {
              closeActionsMenu();
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
      </Menu>
      <Box sx={{ flexGrow: 1 }} />
      {!publication && (
        <Tooltip title={t('records.publication.channelDisabledDescription')}>
          <Button color="inherit" size="small" variant="text">
            {t('records.publication.channelDisabled')}
          </Button>
        </Tooltip>
      )}
      {publication && (
        <Tooltip
          title={
            publication.status === publicationStatuses.published &&
            publication.published_at
              ? t('records.publication.publishedDetails', {
                  publishedAt: formatWithUtc(publication.published_at),
                  publishedBy: publication.published_by_user_id ?? '—',
                })
              : ''
          }
        >
          <Chip
            color={
              publication.status === publicationStatuses.published
                ? 'success'
                : 'default'
            }
            label={t(`records.publication.${publication.status}`)}
            size="small"
          />
        </Tooltip>
      )}
      {publication && notReady && (
        // Focusable so keyboard users can open the reasons; they also form
        // the chip's accessible description.
        <Tooltip describeChild enterTouchDelay={0} title={readinessText}>
          <Chip
            color="warning"
            label={t('records.publication.notReady')}
            size="small"
            tabIndex={0}
          />
        </Tooltip>
      )}
      {canPublish && (
        <>
          <Tooltip
            enterTouchDelay={0}
            title={t('records.publication.actionsDescription')}
          >
            <IconButton
              aria-label={t('records.publication.actionsHelp')}
              size="small"
            >
              <InfoIcon size={compactIconSize} />
            </IconButton>
          </Tooltip>
          <Button
            aria-controls={publishMenuAnchor ? publishMenuId : undefined}
            aria-expanded={Boolean(publishMenuAnchor)}
            aria-haspopup="menu"
            color="inherit"
            disabled={publicationPending}
            endIcon={<ChevronDownIcon />}
            onClick={(event) => setPublishMenuAnchor(event.currentTarget)}
            size="small"
          >
            {t('records.publication.actions')}
          </Button>
          <Menu
            anchorEl={publishMenuAnchor}
            anchorOrigin={{ horizontal: 'right', vertical: 'bottom' }}
            id={publishMenuId}
            onClose={closePublishMenu}
            open={Boolean(publishMenuAnchor)}
            transformOrigin={{ horizontal: 'right', vertical: 'top' }}
          >
            {publication && (
              <MenuItem
                disabled={notReady}
                onClick={() => {
                  closePublishMenu();
                  onPublish();
                }}
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
            <MenuItem
              disabled={notReadyChannels.length > 0}
              onClick={() => {
                closePublishMenu();
                onPublishAll();
              }}
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
            {publication?.status === publicationStatuses.published && (
              <MenuItem
                onClick={() => {
                  closePublishMenu();
                  onUnpublish();
                }}
                sx={{ color: 'warning.main' }}
              >
                <ListItemIcon sx={{ color: 'inherit' }}>
                  <Undo2Icon size={compactIconSize} />
                </ListItemIcon>
                <ListItemText>{t('records.unpublish')}</ListItemText>
              </MenuItem>
            )}
          </Menu>
        </>
      )}
      {!compact && showExtensions && (
        <Tooltip title={t('records.askAboutRecord')}>
          <IconButton
            aria-label={t('records.askAboutRecord')}
            aria-expanded={agentPanelOpen}
            onClick={onOpenAgent}
          >
            <AgentIcon />
          </IconButton>
        </Tooltip>
      )}
      {!compact && showExtensions && (
        <Tooltip title={t('records.extensionContributions')}>
          <IconButton
            aria-controls={RECORD_EXTENSION_DRAWER_ID}
            aria-expanded={extensionPanelOpen}
            aria-label={t('records.extensionContributions')}
            onClick={onOpenExtensions}
          >
            <PanelRightIcon />
          </IconButton>
        </Tooltip>
      )}
    </RecordToolbar>
  );
};
