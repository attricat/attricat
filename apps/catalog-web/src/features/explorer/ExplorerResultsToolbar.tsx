import { Box, Button, IconButton, Tooltip, Typography } from '@mui/material';
import { Columns3CogIcon } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import type { EntityItem } from '../entities/api';
import { maximumAgentSelection } from './agentSelection';
import { ExplorerSelectionActionsMenu } from './ExplorerSelectionActionsMenu';
import { ExplorerSelectionSummary } from './ExplorerSelectionSummary';

type Props = {
  itemCount: number;
  totalCount: number | null;
  totalCountCapped: boolean;
  selectionMode: boolean;
  selectedItems: EntityItem[];
  onClearSelection: () => void;
  onRemoveSelected: (entityId: string) => void;
  onSendSelection: () => void;
  onToggleSelection: () => void;
  onOpenColumnPreferences: () => void;
};

export const ExplorerResultsToolbar = ({
  itemCount,
  totalCount,
  totalCountCapped,
  selectionMode,
  selectedItems,
  onClearSelection,
  onRemoveSelected,
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
        {selectionMode && (
          <ExplorerSelectionSummary
            onClear={onClearSelection}
            onRemove={onRemoveSelected}
            selectedItems={selectedItems}
          />
        )}
        {selectionMode && selectedItems.length >= maximumAgentSelection && (
          <Typography color="text.secondary" variant="caption">
            {t('explorer.selectionLimit', { count: maximumAgentSelection })}
          </Typography>
        )}
      </Box>
      <Box sx={{ alignItems: 'center', display: 'flex', gap: 1 }}>
        {selectionMode && (
          <ExplorerSelectionActionsMenu
            disabled={selectedItems.length === 0}
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
