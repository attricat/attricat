import { Alert, Box, Button, Paper, Typography } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { RelationshipPickerIcon } from '../../components/systemIcons';
import type { RecordItem } from '../records/api';
import { displayLabel } from '../records/recordDisplay';
import { relationshipPickerModes } from '../records/constants';
import {
  relationshipPickerManyMessageType,
  type RelationshipPickerManyMessage,
} from '../records/components/useRecentlyPreviewedRecords';
import { ExplorerSelectionSummary } from './ExplorerSelectionSummary';
import type { RelationshipPickerSession } from './search';

/**
 * Sends the records selected in an Explorer tab back to the relationship
 * picker that opened it, then closes the tab. The picker adds them to its
 * draft, which the user still applies there.
 */
export const RelationshipPickerSelectionBar = ({
  session,
  selectedItems,
  onClear,
  onRemove,
}: {
  session: RelationshipPickerSession;
  selectedItems: RecordItem[];
  onClear: () => void;
  onRemove: (recordId: string) => void;
}) => {
  const { t } = useTranslation();
  const opener = window.opener as Window | null;
  const openerOpen = Boolean(opener && !opener.closed);
  const use = () => {
    if (!opener || opener.closed) return;
    const message: RelationshipPickerManyMessage = {
      type: relationshipPickerManyMessageType,
      token: session.token,
      recordIds: selectedItems.map((item) => item.id),
      labels: Object.fromEntries(
        selectedItems.map((item) => [
          item.id,
          displayLabel(item.display, item.id),
        ]),
      ),
    };
    opener.postMessage(message, window.location.origin);
    window.close();
  };
  return (
    <Paper
      aria-label={t('explorer.relationshipPicker.title')}
      component="aside"
      sx={{
        alignItems: 'center',
        bgcolor: 'action.hover',
        borderColor: 'primary.main',
        display: 'flex',
        flexWrap: 'wrap',
        gap: 1.5,
        mb: 2,
        p: { xs: 1.5, sm: 2 },
      }}
      variant="outlined"
    >
      <RelationshipPickerIcon />
      <Typography sx={{ flexGrow: 1 }} variant="body2">
        {t(
          session.mode === relationshipPickerModes.one
            ? 'explorer.relationshipPicker.chooseOne'
            : 'explorer.relationshipPicker.chooseMany',
        )}
      </Typography>
      {!openerOpen && (
        <Alert severity="warning" sx={{ flexBasis: '100%', order: 1 }}>
          {t('explorer.relationshipPicker.openerClosed')}
        </Alert>
      )}
      <ExplorerSelectionSummary
        onClear={onClear}
        onRemove={onRemove}
        selectedItems={selectedItems}
      />
      <Box sx={{ display: 'flex', gap: 1 }}>
        <Button onClick={() => window.close()} size="small">
          {t('explorer.relationshipPicker.cancel')}
        </Button>
        <Button
          disabled={!openerOpen || selectedItems.length === 0}
          onClick={use}
          size="small"
          variant="contained"
        >
          {t(
            session.mode === relationshipPickerModes.filter
              ? 'explorer.relationshipPicker.useInFilter'
              : 'explorer.relationshipPicker.use',
            { count: selectedItems.length },
          )}
        </Button>
      </Box>
    </Paper>
  );
};
