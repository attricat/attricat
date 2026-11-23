import { Box, Button, Typography } from '@mui/material';
import { Columns3CogIcon } from 'lucide-react';
import type { ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import type { RecordItem } from '../records/api';
import { lexiconCountNoun } from '../lexicon/lexicon';
import { maximumAgentSelection } from './agentSelection';
import { ExplorerSelectionActionsMenu } from './ExplorerSelectionActionsMenu';
import { ExplorerSelectionSummary } from './ExplorerSelectionSummary';
import { LabeledIconButton } from '../../components/LabeledIconButton';
import { useRelationshipPickerSession } from './relationshipPickerSession';

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
  /** Extension actions, shown beside the column preferences. */
  extensionActions?: ReactNode;
  /** Extension actions for the selection, items of its actions menu. */
  selectionExtensionActions?: ReactNode;
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
  extensionActions,
  selectionExtensionActions,
}: Props) => {
  const { t } = useTranslation();
  // A relationship picker tab only selects; its bar sends the selection.
  const picking = useRelationshipPickerSession() !== null;
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
        // On narrow screens the actions wrap below the count and selection.
        flexWrap: 'wrap',
        gap: 1,
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
        {selectionMode && !picking && (
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
      <Box
        sx={{
          alignItems: 'center',
          display: 'flex',
          flexWrap: 'wrap',
          gap: 1,
          ml: 'auto',
        }}
      >
        {selectionMode && !picking && (
          <ExplorerSelectionActionsMenu
            disabled={selectedItems.length === 0}
            extensionActions={selectionExtensionActions}
            onSaveAsSearch={onSaveSelectionAsSearch}
            onSendToAgent={onSendSelection}
          />
        )}
        {!picking && (
          <Button onClick={onToggleSelection} size="small">
            {t(
              selectionMode
                ? 'explorer.exitSelection'
                : 'explorer.selectRecords',
            )}
          </Button>
        )}
        {extensionActions}
        <LabeledIconButton
          label={t('explorer.columnPreferences')}

          onClick={onOpenColumnPreferences}
        >
          <Columns3CogIcon />
        </LabeledIconButton>
      </Box>
    </Box>
  );
};
