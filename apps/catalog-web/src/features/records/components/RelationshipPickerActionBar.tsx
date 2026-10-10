import { Button, Paper } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { RelationshipPickerIcon } from '../../../components/systemIcons';
import { RELATIONSHIP_PICKER_ACTION_MIN_WIDTH } from '../constants';
import { relationshipPickerMessageType } from './useRecentlyPreviewedRecords';

export const RelationshipPickerActionBar = ({
  recordId,
  pickerToken,
}: {
  recordId: string;
  pickerToken: string;
}) => {
  const { t } = useTranslation();
  if (!window.opener) return null;

  const selectAndClose = () => {
    window.opener.postMessage(
      {
        type: relationshipPickerMessageType,
        token: pickerToken,
        recordId,
      },
      window.location.origin,
    );
    window.close();
  };

  return (
    <Paper
      aria-label={t('records.relationshipPickerAction')}
      component="aside"
      sx={{
        alignItems: 'center',
        bgcolor: 'action.hover',
        borderColor: 'primary.main',
        display: 'flex',
        justifyContent: 'center',
        mb: 2,
        p: { xs: 1.5, sm: 2 },
      }}
      variant="outlined"
    >
      <Button
        onClick={selectAndClose}
        size="large"
        startIcon={<RelationshipPickerIcon />}
        sx={{ minWidth: { sm: RELATIONSHIP_PICKER_ACTION_MIN_WIDTH } }}
        variant="contained"
      >
        {t('records.selectThisRecordAndClose')}
      </Button>
    </Paper>
  );
};
