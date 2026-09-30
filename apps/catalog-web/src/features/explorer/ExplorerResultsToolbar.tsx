import { Box, Button, IconButton, Tooltip, Typography } from '@mui/material';
import { Columns3CogIcon, XIcon } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { compactIconSize } from '../../components/iconSizes';
import { maximumAgentSelection } from './agentSelection';
import { ExplorerSelectionActionsMenu } from './ExplorerSelectionActionsMenu';

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
  const total = totalCount ?? itemCount;
  const capped = totalCount !== null && totalCountCapped;
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
        {selectionMode ? (
          <>
            <Typography>
              {t(
                capped
                  ? 'explorer.selectedOfTotalCapped'
                  : 'explorer.selectedOfTotal',
                { selected: selectedCount, total },
              )}
            </Typography>
            {selectedCount > 0 && (
              <Tooltip title={t('explorer.clearSelection')}>
                <IconButton
                  aria-label={t('explorer.clearSelection')}
                  onClick={onClearSelection}
                  size="small"
                >
                  <XIcon size={compactIconSize} />
                </IconButton>
              </Tooltip>
            )}
            {selectedCount >= maximumAgentSelection && (
              <Typography color="text.secondary" variant="caption">
                {t('explorer.selectionLimit', {
                  count: maximumAgentSelection,
                })}
              </Typography>
            )}
          </>
        ) : (
          <Typography>
            {capped
              ? t('explorer.resultCountCapped', { count: total })
              : t('explorer.resultCount', { count: total })}
          </Typography>
        )}
      </Box>
      <Box sx={{ alignItems: 'center', display: 'flex', gap: 1 }}>
        {selectionMode && (
          <ExplorerSelectionActionsMenu
            disabled={selectedCount === 0}
            onSendToAgent={onSendSelection}
          />
        )}
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
            <Columns3CogIcon />
          </IconButton>
        </Tooltip>
      </Box>
    </Box>
  );
};
