import { Box, Button, IconButton, Tooltip, Typography } from '@mui/material';
import { Columns3CogIcon } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import type { RecordItem } from '../records/api';
import { lexiconCountNoun } from '../lexicon/lexicon';
import { maximumAgentSelection } from './agentSelection';
import { ExplorerSelectionActionsMenu } from './ExplorerSelectionActionsMenu';
import { ExplorerSelectionSummary } from './ExplorerSelectionSummary';

type Props = {
  /** Authored blueprint name; a `{{…}}` reference with plural forms names the count. */
  blueprintName: string;
  itemCount: number;
  totalCount: number | null;
  totalCountCapped: boolean;
  selectionMode: boolean;
  selectedItems: RecordItem[];
  onClearSelection: () => void;
  onRemoveSelected: (recordId: string) => void;
  onSaveSelectionAsSearch: () => void;
  onSendSelection: () => void;
  onToggleSelection: () => void;
  onOpenColumnPreferences: () => void;
};

export const ExplorerResultsToolbar = ({
  blueprintName,
  itemCount,
  totalCount,
  totalCountCapped,
  selectionMode,
  selectedItems,
  onClearSelection,
  onRemoveSelected,
  onSaveSelectionAsSearch,
  onSendSelection,
  onToggleSelection,
  onOpenColumnPreferences,
}: Props) => {
  const { t } = useTranslation();
  const resultCount = (count: number, capped: boolean) => {
    const noun = lexiconCountNoun(blueprintName, count);
    if (noun)
      return t(capped ? 'explorer.recordCountCapped' : 'explorer.recordCount', {
        count,
        noun,
      });
    return t(capped ? 'explorer.resultCountCapped' : 'explorer.resultCount', {
      count,
    });
  };
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
            ? resultCount(itemCount, false)
            : resultCount(totalCount, totalCountCapped)}
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
            onSaveAsSearch={onSaveSelectionAsSearch}
            onSendToAgent={onSendSelection}
          />
        )}
        <Button onClick={onToggleSelection} size="small">
          {t(
            selectionMode ? 'explorer.exitSelection' : 'explorer.selectRecords',
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
