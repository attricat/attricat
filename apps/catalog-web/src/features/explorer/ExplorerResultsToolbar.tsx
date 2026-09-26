import SettingsIcon from '@mui/icons-material/Settings';
import { Box, Button, IconButton, Tooltip, Typography } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { maximumAgentSelection } from './agentSelection';

type Props = {
  itemCount: number;
  totalCount: number | null;
  totalCountCapped: boolean;
  selectionMode: boolean;
  selectedCount: number;
  onClearSelection: () => void;
  onSendSelection: () => void;
  onToggleSelection: () => void;
  onOpenColumnPreferences: () => void;
};

export const ExplorerResultsToolbar = ({
  itemCount,
  totalCount,
  totalCountCapped,
  selectionMode,
  selectedCount,
  onClearSelection,
  onSendSelection,
  onToggleSelection,
  onOpenColumnPreferences,
}: Props) => {
  const { t } = useTranslation();
  return (
    <Box
      sx={{
        alignItems: 'center',
        borderBottom: 1,
        borderColor: 'divider',
        display: 'flex',
        justifyContent: 'space-between',
        p: 1,
        pl: 2,
      }}
    >
      <Box
        sx={{
          alignItems: 'center',
          display: 'flex',
          flexWrap: 'wrap',
          gap: 1,
        }}
      >
        <Typography>
          {totalCount === null
            ? t('explorer.resultCount', { count: itemCount })
            : totalCountCapped
              ? t('explorer.resultCountCapped', { count: totalCount })
              : t('explorer.resultCount', { count: totalCount })}
        </Typography>
        {selectionMode && selectedCount > 0 && (
          <>
            <Typography>
              {t('explorer.selectedCount', { count: selectedCount })}
            </Typography>
            <Button onClick={onClearSelection} size="small">
              {t('explorer.clearSelection')}
            </Button>
            <Button onClick={onSendSelection} size="small" variant="contained">
              {t('explorer.sendToAgentConversation')}
            </Button>
          </>
        )}
        {selectionMode && (
          <Typography variant="caption">
            {t('explorer.selectionLimit', { count: maximumAgentSelection })}
          </Typography>
        )}
      </Box>
      <Box sx={{ alignItems: 'center', display: 'flex' }}>
        <Button onClick={onToggleSelection} size="small">
          {t(
            selectionMode
              ? 'explorer.exitSelection'
              : 'explorer.selectEntities',
          )}
        </Button>
        <Tooltip title={t('explorer.columnPreferences')}>
          <IconButton
            aria-label={t('explorer.columnPreferences')}
            onClick={onOpenColumnPreferences}
          >
            <SettingsIcon />
          </IconButton>
        </Tooltip>
      </Box>
    </Box>
  );
};
