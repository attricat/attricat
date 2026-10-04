import {
  ChevronDownIcon,
  CopyIcon,
  GlobeIcon,
  InfoIcon,
  PanelRightIcon,
  PencilIcon,
  RefreshCwIcon,
  RotateCcwClockIcon,
  TrashIcon,
  Undo2Icon,
  UploadIcon,
} from 'lucide-react';
import { AgentIcon } from '../../../components/systemIcons';
import {
  Box,
  Button,
  Chip,
  IconButton,
  ListItemIcon,
  ListItemText,
  Menu,
  MenuItem,
  Tooltip,
} from '@mui/material';
import { useId, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { RouterIconButton } from '../../../components/RouterLink';
import { EntityToolbar } from './EntityToolbar';
import { EntitySchemaStatus } from './EntitySchemaStatus';
import type {
  EntityPublicationReadiness,
  EntityPublicationStatus,
} from '../schemas';
import { ENTITY_EXTENSION_DRAWER_ID, publicationStatuses } from '../constants';
import { publicationReadinessText } from '../checkViolations';
import { compactIconSize } from '../../../components/iconSizes';
import { useInstantFormat } from '../../../time/useInstantFormat';

type Props = {
  entityId: string;
  extensionPanelOpen: boolean;
  onOpenExtensions: () => void;
  agentPanelOpen: boolean;
  onOpenAgent: () => void;
  schemaOutdated?: boolean;
  showExtensions: boolean;
  onDuplicate: () => void;
  duplicatePending: boolean;
  canDelete: boolean;
  onDelete: () => void;
  publication?: EntityPublicationStatus;
  canPublish: boolean;
  /** Check readiness of the selected context's channel. */
  readiness?: EntityPublicationReadiness;
  /** Enabled channels whose checks currently fail. */
  notReadyChannels?: readonly EntityPublicationReadiness[];
  onPublish: () => void;
  onPublishAll: () => void;
  onUnpublish: () => void;
  publicationPending: boolean;
};

export const EntityPreviewToolbar = ({
  entityId,
  extensionPanelOpen,
  onOpenExtensions,
  agentPanelOpen,
  onOpenAgent,
  schemaOutdated,
  showExtensions,
  onDuplicate,
  duplicatePending,
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
  // A tooltip cannot host another tooltip, so the UTC value is inline.
  const { formatWithUtc } = useInstantFormat();
  const publishMenuId = useId();
  const [publishMenuAnchor, setPublishMenuAnchor] =
    useState<HTMLElement | null>(null);
  const closePublishMenu = () => setPublishMenuAnchor(null);
  const notReady = readiness?.ready === false;
  const readinessText = readiness && publicationReadinessText(readiness);
  return (
    <EntityToolbar label={t('entities.entityPreview')}>
      <Tooltip title={t('entities.editEntity')}>
        <RouterIconButton
          aria-label={t('entities.editEntity')}
          params={{ entityId }}
          to="/entities/$entityId/edit"
        >
          <PencilIcon />
        </RouterIconButton>
      </Tooltip>
      <Tooltip title={t('entities.changes')}>
        <RouterIconButton
          aria-label={t('entities.changes')}
          params={{ entityId }}
          to="/entities/$entityId/changes"
        >
          <RotateCcwClockIcon />
        </RouterIconButton>
      </Tooltip>
      <Tooltip title={t('entities.duplicateEntity')}>
        <span>
          <IconButton
            aria-label={t('entities.duplicateEntity')}
            disabled={duplicatePending}
            onClick={onDuplicate}
          >
            <CopyIcon />
          </IconButton>
        </span>
      </Tooltip>
      {schemaOutdated !== undefined && (
        <EntitySchemaStatus
          entityId={entityId}
          schemaOutdated={schemaOutdated}
        />
      )}
      {canDelete && (
        <Tooltip title={t('entities.deleteEntity')}>
          <IconButton
            aria-label={t('entities.deleteEntity')}
            color="error"
            onClick={onDelete}
          >
            <TrashIcon />
          </IconButton>
        </Tooltip>
      )}
      <Box sx={{ flexGrow: 1 }} />
      {!publication && (
        <Tooltip title={t('entities.publication.channelDisabledDescription')}>
          <Button color="inherit" size="small" variant="text">
            {t('entities.publication.channelDisabled')}
          </Button>
        </Tooltip>
      )}
      {publication && (
        <Tooltip
          title={
            publication.status === publicationStatuses.published &&
            publication.published_at
              ? t('entities.publication.publishedDetails', {
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
            label={t(`entities.publication.${publication.status}`)}
            size="small"
          />
        </Tooltip>
      )}
      {publication && notReady && (
        <Tooltip enterTouchDelay={0} title={readinessText}>
          <Chip
            color="warning"
            label={t('entities.publication.notReady')}
            size="small"
          />
        </Tooltip>
      )}
      {canPublish && (
        <>
          <Tooltip
            enterTouchDelay={0}
            title={t('entities.publication.actionsDescription')}
          >
            <IconButton
              aria-label={t('entities.publication.actionsHelp')}
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
            {t('entities.publication.actions')}
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
                    <UploadIcon size={compactIconSize} />
                  ) : (
                    <RefreshCwIcon size={compactIconSize} />
                  )}
                </ListItemIcon>
                <ListItemText secondary={notReady ? readinessText : undefined}>
                  {publication.status === publicationStatuses.notPublished
                    ? t('entities.publish')
                    : t('entities.republish')}
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
                    ? t('entities.publication.channelsNotReady', {
                        channels: notReadyChannels
                          .map((channel) => channel.context_code)
                          .join(', '),
                      })
                    : undefined
                }
              >
                {t('entities.publishAllChannels')}
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
                <ListItemText>{t('entities.unpublish')}</ListItemText>
              </MenuItem>
            )}
          </Menu>
        </>
      )}
      {showExtensions && (
        <Tooltip title={t('entities.askAboutEntity')}>
          <IconButton
            aria-label={t('entities.askAboutEntity')}
            aria-expanded={agentPanelOpen}
            onClick={onOpenAgent}
          >
            <AgentIcon />
          </IconButton>
        </Tooltip>
      )}
      {showExtensions && (
        <Tooltip title={t('entities.extensionContributions')}>
          <IconButton
            aria-controls={ENTITY_EXTENSION_DRAWER_ID}
            aria-expanded={extensionPanelOpen}
            aria-label={t('entities.extensionContributions')}
            onClick={onOpenExtensions}
          >
            <PanelRightIcon />
          </IconButton>
        </Tooltip>
      )}
    </EntityToolbar>
  );
};
