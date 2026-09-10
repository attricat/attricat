import CheckCircleOutlinedIcon from '@mui/icons-material/CheckCircleOutlined';
import EditOutlinedIcon from '@mui/icons-material/EditOutlined';
import HistoryOutlinedIcon from '@mui/icons-material/HistoryOutlined';
import UpgradeOutlinedIcon from '@mui/icons-material/UpgradeOutlined';
import ViewSidebarOutlinedIcon from '@mui/icons-material/ViewSidebarOutlined';
import WarningAmberOutlinedIcon from '@mui/icons-material/WarningAmberOutlined';
import { Box, IconButton, Tooltip } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { RouterIconButton } from '../../../components/RouterLink';
import { EntityToolbar } from './EntityToolbar';

type Props = {
  entityId: string;
  extensionPanelOpen: boolean;
  onOpenExtensions: () => void;
  schemaOutdated?: boolean;
  showExtensions: boolean;
};

export const EntityPreviewToolbar = ({
  entityId,
  extensionPanelOpen,
  onOpenExtensions,
  schemaOutdated,
  showExtensions,
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
