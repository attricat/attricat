import ContentCopyOutlinedIcon from '@mui/icons-material/ContentCopyOutlined';
import DeleteOutlinedIcon from '@mui/icons-material/DeleteOutlined';
import EditOutlinedIcon from '@mui/icons-material/EditOutlined';
import HistoryOutlinedIcon from '@mui/icons-material/HistoryOutlined';
import ViewSidebarOutlinedIcon from '@mui/icons-material/ViewSidebarOutlined';
import { AgentIcon } from '../../../components/systemIcons';
import { Box, Button, IconButton, Tooltip } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { RouterIconButton } from '../../../components/RouterLink';
import { EntityToolbar } from './EntityToolbar';
import { EntitySchemaStatus } from './EntitySchemaStatus';
import type { EntityPublicationStatus } from '../schemas';
import { ENTITY_EXTENSION_DRAWER_ID, publicationStatuses } from '../constants';

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
  onPublish,
  onPublishAll,
  onUnpublish,
  publicationPending,
}: Props) => {
  const { t } = useTranslation();
  return (
    <EntityToolbar label={t('entities.entityPreview')}>
      <Tooltip title={t('entities.editEntity')}>
        <RouterIconButton
          aria-label={t('entities.editEntity')}
          params={{ entityId }}
          to="/entities/$entityId/edit"
        >
          <EditOutlinedIcon />
        </RouterIconButton>
      </Tooltip>
      <Tooltip title={t('entities.changes')}>
        <RouterIconButton
          aria-label={t('entities.changes')}
          params={{ entityId }}
          to="/entities/$entityId/changes"
        >
          <HistoryOutlinedIcon />
        </RouterIconButton>
      </Tooltip>
      <Tooltip title={t('entities.duplicateEntity')}>
        <span>
          <IconButton
            aria-label={t('entities.duplicateEntity')}
            disabled={duplicatePending}
            onClick={onDuplicate}
          >
            <ContentCopyOutlinedIcon />
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
            <DeleteOutlinedIcon />
          </IconButton>
        </Tooltip>
      )}
      <Box sx={{ flexGrow: 1 }} />
      {publication ? (
        <Tooltip
          title={
            publication.status === publicationStatuses.published &&
            publication.published_at
              ? t('entities.publication.publishedDetails', {
                  publishedAt: new Date(
                    publication.published_at,
                  ).toLocaleString(),
                  publishedBy: publication.published_by_user_id ?? '—',
                })
              : t(`entities.publication.${publication.status}`)
          }
        >
          <Button color="inherit" size="small" variant="text">
            {t(`entities.publication.${publication.status}`)}
          </Button>
        </Tooltip>
      ) : (
        <Tooltip title={t('entities.publication.channelDisabledDescription')}>
          <Button color="inherit" size="small" variant="text">
            {t('entities.publication.channelDisabled')}
          </Button>
        </Tooltip>
      )}
      {canPublish &&
        publication &&
        (publication.status === publicationStatuses.notPublished ? (
          <Button
            disabled={publicationPending}
            onClick={onPublish}
            size="small"
          >
            {t('entities.publish')}
          </Button>
        ) : (
          <Button
            color="warning"
            disabled={publicationPending}
            onClick={onUnpublish}
            size="small"
          >
            {t('entities.unpublish')}
          </Button>
        ))}
      {canPublish && (
        <Button
          disabled={publicationPending}
          onClick={onPublishAll}
          size="small"
        >
          {t('entities.publishAllChannels')}
        </Button>
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
            <ViewSidebarOutlinedIcon />
          </IconButton>
        </Tooltip>
      )}
    </EntityToolbar>
  );
};
