import CheckCircleOutlinedIcon from '@mui/icons-material/CheckCircleOutlined';
import EditOutlinedIcon from '@mui/icons-material/EditOutlined';
import HistoryOutlinedIcon from '@mui/icons-material/HistoryOutlined';
import UpgradeOutlinedIcon from '@mui/icons-material/UpgradeOutlined';
import ViewSidebarOutlinedIcon from '@mui/icons-material/ViewSidebarOutlined';
import WarningAmberOutlinedIcon from '@mui/icons-material/WarningAmberOutlined';
import { Box, Button, IconButton, Tooltip } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { RouterIconButton } from '../../../components/RouterLink';
import { EntityToolbar } from './EntityToolbar';
import type { EntityPublicationStatus } from '../schemas';

type Props = {
  entityId: string;
  extensionPanelOpen: boolean;
  onOpenExtensions: () => void;
  schemaOutdated?: boolean;
  showExtensions: boolean;
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
  schemaOutdated,
  showExtensions,
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
      {schemaOutdated !== undefined &&
        (schemaOutdated ? (
          <>
            <Tooltip title={t('entities.schemaOutdated')}>
              <WarningAmberOutlinedIcon color="warning" fontSize="small" />
            </Tooltip>
            <Tooltip title={t('entities.upgradeBlueprint')}>
              <RouterIconButton
                aria-label={t('entities.upgradeBlueprint')}
                params={{ entityId }}
                to="/entities/$entityId/migrate"
              >
                <UpgradeOutlinedIcon />
              </RouterIconButton>
            </Tooltip>
          </>
        ) : (
          <Tooltip title={t('entities.matchesCurrentSchema')}>
            <CheckCircleOutlinedIcon color="success" fontSize="small" />
          </Tooltip>
        ))}
      <Box sx={{ flexGrow: 1 }} />
      {publication ? (
        <Tooltip
          title={
            publication.status === 'published' && publication.published_at
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
        (publication.status === 'not_published' ? (
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
        <Tooltip title={t('entities.extensionContributions')}>
          <IconButton
            aria-controls="entity-extension-contributions"
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
